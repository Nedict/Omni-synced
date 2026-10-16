# Small editable history fixtures

These 177 designs and their JSON command recipes are original SolidCraft test
data. They were authored through SolidCraft commands; no Fusion-produced file is
included. Each `.solidcraft` can be opened normally. Each `.json` contains its
recipe, independent expectations and optional edits.

`legacy/offset-circle-ring.solidcraft` retains the development branch's expression-driven
offset. Its compatibility test checks the same analytic volumes, edits, reopening and
undo after upstream introduced a driving offset dimension. The main fixture is authored
with the current command behavior. Development formats 14–22 use the explicit
`solidcraft-f3d-dev/N` tag to distinguish them from upstream's format numbering.

| Fixture | Features checked | Analytic volume (mm³) |
|---|---|---:|
| circle-extrude | Driving diameter, height expression | 5040π |
| circular-symmetric-body | Even-count symmetric body copies; count, signed/full sweep and height edits; volume, bounds and center of mass | 360π |
| circular-symmetric-feature | Odd-count symmetric feature copies; count, signed/full sweep and height edits; volume, bounds and center of mass | 450π |
| user-parameters | User/model roles, comments, favorites, dependent height | 4000π |
| negative-extrude | Signed height, downward bounds | 5040π |
| symmetric-extrude-half | Per-side distance, centered bounds, diameter/height edits | 10080π |
| symmetric-extrude-full | Full-length parameter driving both half-extents, centered bounds | 5040π |
| grouped-profile-union | Three explicitly selected circle regions merge into one body; diameter/height edits, analytic union volume and bounds | (1800π − lens(30,30,40)) × 17 |
| coincident-profile-lens | A lens retains both coincident source circles; shared diameter and height edits, undo and reopening | lens(30,30,40) × 17 |
| subdivided-profile-lens | An interior disk and its surrounding lens region remain one filled extrusion; source, height and divider edits | lens(30,30,40) × 17 |
| placed-circle | Offset local center, explicit XZ placement | 324π |
| plane-xy | Offset plane, parent edits and negative placement | 1500π |
| plane-yz | YZ plane offset toward +X, parameter and feature edits | 1500π |
| plane-xz | XZ plane offset toward +Y, parent edits | 1500π |
| plane-chained | Two dependent planes, parameter and feature edits | 1500π |
| suppressed-extrude | Suppression and unsuppression | 0 → 5040π |
| new_body-extrude | Two independent bodies | 5340π |
| join-extrude | Contained cylinder joined into base | 5040π |
| cut-extrude | Blind circular pocket | 4740π |
| intersect-extrude | Intersection with smaller cylinder | 300π |
| multiple-circles | Two selected regions, separate bodies, diameter/height edits | 6000π |
| multiple-rings | Two regions with separate holes, diameter/height edits | 4500π |
| partial-lens | Overlapping circles, exact arc pieces, diameter/height edits | 12 × lens area |
| partial-left-crescent | Left circle minus lens, opposite arc traversal | 12 × (900π − lens area) |
| partial-right-crescent | Right circle minus lens, diameter/height edits | 12 × (900π − lens area) |
| partial-left-rectangle | Divider middle piece, source width and height edits | 9600 |
| partial-right-rectangle | Opposite divider traversal, width and height edits | 19200 |
| offset-circle-ring | Associative offset distance, source diameter and height edits | 3300π |
| annulus-extrude | Exact outer/hole boundaries, both diameter edits | 9600π |
| arc-extrude | Shared arc/chord endpoints, radius and height edits | 1350π |
| rectangle-extrude | Width/depth dimensions, dependent expression | 10200 |
| projected-circle | Source sketch link, diameter edits, extrusion | 5040π |
| projected-body-edge | Persistent edge of one of two overlapping bodies, large edits | 7920π |
| projected-body-circle | Both named circular boundary edges retained as one circle | 7920π |
| centerline-extrude | Axis crossing a rectangle stays out of profiles, dimension edits | 10200 |
| aligned-dimension | Width and diagonal distance, dependent side and bounds | 12000 |
| rectangle-negative | Rectangular prism with downward height | 10200 |
| reordered-sketch | Feature order differs from object ID order | 5340π |
| projected-body-rectangle | Persistent shared corners, width/depth/source-height edits | 33000 |
| extrusion-start-offset | Signed start offset and dependent height expression | 10200 |
| symmetric-cylinders | Center symmetry, equal radii, diameter/height edits | 6000π |
| offset-dimension-prism | Parallel-line distance, width/depth/height edits | 10200 |
| line-midpoint-prism | Shared line centers, moving reference, width/depth/height edits | 10200 |
| fixed-arc-support | Fixed center/radius with moving endpoint, height edits | 4500(acos(1/3) − √8/3) |
| angular-prism | Driving angle, supplementary angles, height edits | 10200 |

The fixed arc support uses radius 30 mm, a fixed start at (30, 0), and an endpoint
whose X coordinate changes from 10 to 20 or 5 mm. Its segment volume is
r²(θ − sin θ)h/2, where θ = acos(endpoint_x/r). Endpoint edits preserve the
center, radius and start while changing the segment bounds and volume.

The line-midpoint prism shares the bottom edge center with an 80 mm construction
line through a free midpoint. Width and depth edits change volume; moving the
reference line changes bounds while preserving volume. Both behaviors must
survive reopening and undo.

The annulus uses radii 30 and 10 mm with height 12 mm, giving π(R² − r²)h.
The semicircular segment uses radius 15 mm and height 12 mm, giving πr²h/2.

The body-edge fixture has three independent overlapping cylinders, all with
radius 12 mm and heights 35, 11 and 9 mm. The last circle projects the top edge
of the first extrusion, selecting both semicircular boundary edges. Their summed
volume is πr²(35 + 11 + 9). Increasing
diameter to 120 mm and reducing the first height to 5 mm exercise persistent
edge selection beyond the original position-based search tolerance.
The `projected-body-circle` variant uses `sketch.project` with `circle: true`,
and additionally verifies that the projection remains one curve and one link.

The rectangular prisms are 30 × 20 × 17 mm; expectations use width × depth × height.
The aligned-dimension prism has width 30 mm, diagonal 50 mm and height 10 mm.
Its depth follows √(diagonal² − width²); diagonal and width edits check both
volume and bounds, including after reopening and undo.

The plane fixtures use radius 10 mm and height 15 mm. Editing their parent plane
changes world bounds while preserving volume. Bounds are checked after each edit,
reopening and undo, so a frozen sketch placement cannot pass on volume alone.

The base cylinder has radius 12 mm and height 35 mm. The second cylinder has
radius 5 mm and height 12 mm. Expectations use πr²h and sum/difference, not output
recorded from the implementation. The placed cylinder has radius 6 mm, height
9 mm and local center (7, -4); its local Y axis points toward world -Z.

`editable_history.rs` replays each recipe, compares the saved document, opens it
through `doc.open`, checks feature order, body count, aggregate world bounds and
volume, then checks edits, JSON and container serialization/reopening, and undo.
The checked-in JSON designs retain their historical format versions; each baseline
and edited document must also survive the current container format with identical
parameters and references. Curved-volume checks
use the existing oracle tolerance of 0.1% because `inspect.measure` integrates
tessellated faces; world bounds use 0.00001 mm.

```sh
cargo test --profile ci -p solidcraft-cli --test editable_history
```

To deliberately regenerate these contemporary fixtures after changing a recipe:

```sh
cargo test --profile ci -p solidcraft-cli --test editable_history write_original_history_fixtures -- --ignored
```

These are native SolidCraft history regression tests. The separate F3D tests
check native record decoding against local Fusion API controls. Passing these
fixtures does **not** establish general F3D editable-history support.

The offset ring uses an outer radius of 30 mm and an inward gap of 5 mm,
with height 12 mm: π(R² − (R − gap)²)h. The gap expression and source
diameter remain editable; each change is checked after reopening and undo.

The multiple-region fixtures extrude circles of radii 10 and 20 mm by 12 mm.
The ring variant has holes of radii 5 and 10 mm, giving
π((10² − 5²) + (20² − 10²))12 = 4500π mm³. Diameter and height edits
check both bodies and their bounds through reopening and undo.

The partial-circle fixtures use radii 30 mm and center separation 40 mm. Their
baseline lens area is `1800 × acos(2/3) − 400 × sqrt(5)` mm². The diameter edit
uses the general two-circle intersection formula with radii 35 and 30 mm;
expectations are analytic values, not captured kernel measurements.

Five `pieces-*` fixtures additionally cover a rotated-seam lens and crescent, a
quarter-circle with cuts exactly on its seam, and both regions of a split
semicircle (including a rotated source arc). Their source radius/diameter and
height edits survive reopening and undo. Quarter areas are πr²/4. The smaller
semicircle region has area `(r² acos(x/r) − x sqrt(r² − x²))/2`, with divider
x = 10 mm; the larger region is the semicircle area minus that value.

Three `loft-*` fixtures add circular and rectangular transitions between two
sketches, including reversed section order. Source dimensions and section-plane
height stay editable through reopening and undo. The circular frustum uses
πh(r₀² + r₀r₁ + r₁²)/3. The rectangular transition uses
h(2w₀d₀ + w₀d₁ + w₁d₀ + 2w₁d₁)/6; both formulas are independent of the kernel.

Three `loft-face-*` fixtures use the top or bottom cap of a 20 mm diameter,
10 mm tall extrusion as a persistent section. The other section is a 40 mm
diameter circle 30 mm away. NewBody and Join controls have the same analytic
volume, 8000π mm³, with different body counts. Independent source diameter,
parent height and section-plane edits check that the loft follows the original
face after regeneration, saving and undo. The bottom-face control also guards
against circular loft winding being lost between oppositely oriented planes.

The three `loft-face-rectangle-*` fixtures add the corresponding rectangular
cases. A 10 × 20 × 10 mm pedestal contributes 2000 mm³; its loft to a 20 × 40 mm
section 30 mm away contributes 14000 mm³, using the rectangular transition
formula above. Width, parent-height and plane-height edits check analytic volume
and bounds independently, including the downward loft from the bottom face.


Three `loft-three-circle-*` fixtures use radii 7, 16 and 11 mm on planes at
0, 23 and 71 mm. Forward and reversed circular-chord lofts have the same volume;
the legacy uniform fixture intentionally differs. Expected volumes are analytic
integrals of `pi * r(u)^2 * z'(u)` for the quadratic interpolants, not values
captured from either CAD application's tessellation. Each recipe checks diameter
and middle-plane edits independently, then reopen and undo. These fixtures bring
the suite to 55 original saved designs.


`loft-face-joined-side` adds a side-face attachment after two same-profile
extrusions, the second a Join. Its footprint is 10 x 20 mm and its final height
10 mm. A loft from the +X face to a 20 x 30 mm rectangle 30 mm away contributes
12000 mm³; the pedestal contributes 2000 mm³. Width, Join-height and destination-
plane edits check volumes 13000, 16500 and 18000 mm³ and exact bounding boxes.
The saved face certificate retains both contributing operations. There are now
56 original saved designs at that milestone.

`rotated-offset-sketch` is a 10 x 20 x 5 mm prism on an offset YZ plane whose
sketch axes are rotated -90 degrees about X. Its 1000 mm³ volume becomes 1500
when the width changes to 15 mm, or 1600 when the extrusion changes to 8 mm.
Moving the support plane from 20 to 30 mm retains volume and translates the
exact bounds. Reopen and undo preserve all three dependencies. This brings the
suite to 57 original saved designs.


`loft-face-mixed-sketches` uses the same analytic dimensions as the joined-side
fixture, with the second extrusion coming from a separate sketch. Both source
sketches depend on the same width parameter; the saved certificate retains their
distinct feature and curve identities through edits, reopen and undo.

`loft-face-polygon-side` extrudes an eight-line stepped footprint of area 800 mm²
by 10 mm. Its outer +X side is a 10 x 10 mm rectangle at X = 40 mm. A loft to a
30 x 40 mm section at X = 60 mm contributes 11000 mm³, giving 19000 mm³ total.
Increasing the pedestal height to 15 mm gives 23833⅓ mm³; moving the destination
to X = 70 mm gives 24500 mm³. These independent expectations use the rectangular
transition formula above and include aggregate bounds. There are 59 saved designs.


`loft-join-mixed-sketches` and `loft-join-polygon-side` use the same independently
calculated dimensions and volumes as their two-body counterparts, but fuse the
transition into the existing body. They must remain one body after width, parent
height and destination-plane edits, reopening and undo. The suite contains 61
original saved designs.




`loft-serial-join` and `loft-triple-join` extend the rectangular joined pedestal
with one or two additional transitions, each 20 mm tall. The successive sections
are 20 x 40, 30 x 50 and 40 x 60 mm. Their added analytic volumes are 22666⅔ and
38666⅔ mm³, giving totals 38666⅔ and 77333⅓ mm³. The chained planes follow edits
to the first destination height. Source width, pedestal height and destination
height edits check volume, exact bounds and one body through reopen and undo.
The suite contains 63 original saved designs.


`extrusion-to-cap`, `extrusion-to-bottom` and `extrusion-to-plane` cut a 10 x 10 mm
pocket into a 40 x 30 x 30 mm block, ending 5 mm short of an exact face or a live
construction plane. Cap/bottom volumes are 33500 mm³; the plane lies 10 mm before
the cap and gives 34500 mm³. Height, pocket-width and endpoint-offset edits use
`1200 * height - 10 * width * (height - plane_gap + offset)` and exact bounds.
All cases retain one body through reopen and undo. There are 68 original models.
The `to_plane` command input accepts a construction-plane name or `{face:[x,y,z]}`;
face selection is saved as a certified attachment, not a frozen point.

`extrusion-travel-cap` and `extrusion-travel-negative-cap` retain the extrusion's
logical end rather than its initial low/high cap. Reversing the parent height
from 30 to -40 mm moves the pocket to the opposite side of XY. Both models check
44500 mm³ after reversal, plus independent width and endpoint-offset edits,
world bounds, reopening and undo.

`mirror-body-origin`, `mirror-body-plane` and `mirror-body-xz` reflect a cylinder
of radius 10 mm and height 30 mm as a separate body. Their total volume is
2πr²h = 6000π mm³; diameter and height edits give 13500π and 8000π respectively.
The offset plane moves from Z = 50 to -20 mm, moving the reflected cylinder from
Z = 70..100 to -70..-40 without changing volume. The XZ case reflects a source
centered at Y = 20 to Y = -20. Suppression retains only the original cylinder.
Every case checks exact bounds, body count, volume, reopening and undo.

`mirror-feature-new`, `mirror-feature-join` and `mirror-feature-cut` reflect an
extrusion recipe across a live YZ offset plane. A circle with radius 5 mm, centered
at X = 20 mm, creates a 30 mm extrusion or a 10 mm deep cut. The New Body case
contains two cylinders with volume 1500π mm³. Join and Cut operate on an
80 x 60 x 20 mm block, giving 96000 + 500π and 96000 − 500π mm³ respectively.
Independent diameter, extrusion distance, plane offset and suppression edits
check analytic volume, body count and bounds through reopening and undo.
The suite now contains 74 original saved designs.

Six `combine-*` designs use two 20 × 20 × 20 mm blocks overlapping by 10 mm
along X. Their volumes are 8000 mm³ each and their intersection is 4000 mm³:
Join gives 12000 mm³, Cut and Intersect give 4000 mm³. Keeping the tool adds
8000 mm³ and retains a second body. Independent target width and both extrusion
height edits recalculate the intersection as overlap-width × 20 × min(heights).
Suppression restores both source bodies. Every case checks exact world bounds,
body count, volume, reopening and undo. These bring the suite to 80 saved designs.

Twenty-four `combine-source-*` designs extend the cube controls with a prior
10 mm extension, a 10 × 10 × 10 mm pocket, or a reflection through X = 15 mm.
Join/Cut/Intersect volumes are 12000/4000/8000 mm³ for the extended source,
11500/3500/3500 mm³ for the pocketed source, and 12000/4000/4000 mm³ for
mirrored sources. Keeping the tool adds 8000 mm³. Swapping the mirror into
the target role moves the Cut result to X = 20..30 mm. Source-height edits,
mirror-plane moves to X = 17 mm, and suppression are checked analytically,
including exact bounds, reopening and undo. There are 104 saved designs.

Four `fillet-*` examples round a vertical edge, the opposite vertical pair, or
one top/bottom edge of a 20 × 30 × 40 mm extrusion. Their volume is
`width × depth × height − (1 − π/4) × radius² × selected_edge_length`.
Radius, source height, width and suppression edits check volume and bounds
through reopening and undo. The extra volume tolerance scales with removed
material, so a wrong edge cannot hide within the much larger block volume.
There are 108 original saved designs.


Four `fillet-groups-*` examples retain independent radii within one feature:
two opposite edges, three corners, two edges per radius, and opposite top/bottom
edges. Their volume is `width × depth × height − (1 − π/4) × Σ(radius² × edge_length)`.
Each radius is edited independently; width, height and suppression edits also
check bounds and volume through reopening and undo. Group parameters retain
their identity when the feature is redefined. There are 112 original saved designs.


Four `fillet-chain-*` fixtures preserve tangent-chain choices, including a
single selected edge of a rounded-slot rim, a circle rim, opposite cap directions
and mixed settings across radius groups. For a convex profile with area A and
perimeter P, a rim of radius r removes
`P × r² × (1 − π/4) − π × r³ × (5/3 − π/2)`.
This integrates the parallel area `A − P × s + π × s²` through the quarter-circle
blend. Slot profiles use `A = 2RL + πR²` and `P = 2L + 2πR`; circles use L = 0.
Radii, extrusion heights, circle diameter and suppression checks also verify
bounds, JSON/container round trips and undo. Their curved-wall allowance is
0.02% of the unrounded volume plus 1.5% of the removed volume and 0.1 mm³;
this still rejects a missing or partially rounded rim. There are 116 designs.

Eight `chamfer-*` examples bevel a vertical edge, the opposite vertical pair,
or a top/bottom edge of a 20 × 30 × 40 mm extrusion. The top examples also
cover two setbacks and distance/angle, with each possible side assignment.
An edge of length L with perpendicular setbacks a and b removes a triangular
prism of volume `L × a × b / 2`; angle mode uses `b = a × tan(angle)`.
Each removed triangle has its centroid one third of the way along each setback.
The remaining center of mass is `(V_block × C_block − V_removed × C_removed)
/ (V_block − V_removed)`. Opposite vertical removals have the block's centroid.
Center-of-mass checks (1e-6 mm) distinguish flipped sides even when volume and
bounds are identical. Volume checks use 0.01 mm³ for these planar solids.
First/second setback, angle, width, height and suppression edits pass through
JSON/container reopening and undo. There are 124 original saved designs.


Four further chamfer fixtures cover independent equal-distance groups, mixed
angle/equal groups with flipped sides, and persistent circular tangent chains
on one or both caps. The planar fixtures use the triangular-prism volume and
centroid formulas above, summed over groups. A full circular bevel of setback d
on a cylinder of radius R removes `π × (R × d² − d³/3)`, from integrating its
annular sections. Circular measurements allow 0.04% for tessellation; a missing
half-rim exceeds that allowance. Independent group dimensions, source height
and width/diameter, suppression, JSON/container reopening and undo are checked.
There are 128 original saved designs.


`subdivided-profile-lens` selects the lens of two radius-30 circles whose centers
are 40 mm apart, including a disk and the surrounding annular region created by
an interior circle. Its extrusion is one filled body. Volume uses the independent
circle-intersection formula (lens area times height), with height, source diameter
and internal-divider diameter edits, save/reopen and undo checks. The divider
diameter changes without changing the union's volume. There are 134 saved designs.

The six `signed-extrude-*` fixtures extrude a 20 × 30 mm rectangle by −40 mm
and apply one fillet or chamfer to a vertical edge, the sketch-plane cap, or the
travel cap. Fillets remove a prism with area `r²(1 − π/4)`; its inward centroid
is `r(5/6 − π/4)/(1 − π/4)`. Chamfers remove triangular prisms with area `d²/2`
and centroid `d/3`. Subtracted volume moments distinguish the two caps even
when volume and bounds match. Radius/setback, negative depth, width, suppression,
reopening and undo are checked. The suite now contains 143 original saved designs.

The two `mixed-profile-*` examples select the intersection of a radius-30 circle
and the rectangle x∈[−5,5], y∈[−35,−25], then extrude −40 mm and blend the two
vertical rectangle corners. The selected area is
`2 a t + a sqrt(R²−a²) + R² asin(a/R)`, with `a=5`, `t=−25`.
Its first y-moment is `(t²−R²)a + a³/3`. Fillet/chamfer removals and their moments
use the corner formulas above. Setback/radius, depth, circle diameter, suppression,
undo and reopening are checked. There are now 145 original saved designs.

`mixed-profile-fillet-pattern` repeats the rounded mixed-profile prism four times
about Z. Its volume is four times the independent segment/corner formula and its
aggregate centroid lies on Z. Radius, depth, diameter, blend suppression and eight
copies are checked with reopening and undo. There are 146 original saved designs.

`combine-circular-pattern-{keep,consume}` cuts four radius-3 tools on a radius-20
Z pattern from a radius-40, height-10 cylinder. The final stock volume is
`(40² − 4 × 3²) × 10 × π`; keeping the tools restores the aggregate volume to
`40² × 10 × π`. Height, tool diameter and Combine suppression are checked,
including centroid, bounds, body count, undo and reopening. There are 148
original saved designs.

`combine-circular-small-grips` joins ten rounded grips to a 15 × 15 × 10 mm
stock with a radius-3 opening. Each grip is the circle intersected with
x∈[−0.48,0.02], y∈[−3.5,−2.5], extruded 12 mm down, with two radius-0.1
vertical corner fillets. Its area is `−2.5 × 0.5 + F(0.02) − F(−0.48)
− 2r²(1−π/4)`, where `F(x) = (x sqrt(9−x²) + 9 asin(x/3))/2`.
The total volume adds ten such prisms to `(225−9π) × 10`; symmetry fixes the
horizontal centroid and the two extrusion depths determine its vertical moment.
Depths 10 and 14, radius 0.2, Combine suppression, undo and reopening are checked.
Joined volumes use a 0.02 mm³ tolerance. With Combine suppressed, the standalone
stock has no small fillet to tighten its measurement tessellation; the 0.2 mm³
tolerance covers its circular-hole chord error and the separate grip surfaces.
This covers the prism-axis search after a pattern introduces more than eight
wall directions. There are 149 original saved designs.

`circular-blend-{fillet,chamfer}-bottom` selects both halves of a circular rim
without tangent-chain expansion. The radius-10, height-40 cylinder retains
diameter, height, blend size and suppression edits. Volume follows the circular
rim formulas above. For a bottom chamfer, removed material has first z-moment
`π(Rr³/3 − r⁴/12)`; for a bottom fillet it is
`r × removed_volume − π(Rr³/3 − r⁴/12)`. Subtracting these moments checks that
the bottom cap, rather than the top cap, remains selected after edits and
reopening. These two examples brought the saved suite to 151 designs.

The two `circular-asymmetric-*-bottom` examples retain separate setback dimensions
(two distances or distance and angle), with the first distance on the cylinder
wall. For radial setback a and axial setback b, removed volume is
πab(R − a/3); its first axial moment at the bottom is πab²(R/3 − a/12).
Tests edit each setback, diameter and height, suppress the feature, undo and reopen.
Kernel and native-import tests separately verify the cap radius and wall contact
height, so exchanged setbacks cannot hide within volume tolerances.

The `face-extrude-chamfer-top` and `face-extrude-chamfer-bottom` examples add
a joined extension from a chamfer-trimmed circular cap. The face profile remains
linked; edits to diameter, source height, both chamfer distances and extension
length are checked against cylinder/frustum volume formulas, along with bounds,
suppression, undo and reopening. These are original SolidCraft recipes.

The `face-cut-chamfer-top` and `face-cut-chamfer-bottom` examples cut 5 mm into
a trimmed cap of a diameter-20, height-40 cylinder. The cap and wall chamfer
setbacks are 2 and 3 mm. For radius R, radial setback a, axial setback b and
pocket depth d, volume is `πR²H − πab(R − a/3) − π(R − a)²d`.
Independent first moments check the pocket's centroid at the selected end.
Diameter, height, both setbacks, depth, suppression, undo and reopening are
covered. These bring the original saved design suite to 157 examples.

The four `projected-face-{extrude,cut}-chamfer-{top,bottom}` examples project
the circular cap of a face extrusion or pocket into a linked sketch and extrude
that sketch 7 mm into a second body. The projection follows diameter, height,
both chamfer setbacks and extension/depth edits. Analytic volume and first
moments include the added radius-(R − a), height-7 cylinder. Bottom examples use radial
setback 3 mm and axial setback 2 mm; top examples use 2 mm and 3 mm. Suppression, undo
and container reopening are checked too. The saved suite has 161 examples.

The two `projected-face-cut-plain-{top,bottom}` examples remove 5 mm from the
full cap of a radius-10, height-40 cylinder, project the new circular boundary,
and extrude a separate 7 mm body. Total volume is `πR²(H − depth + 7)`.
Independent centroid and bounds checks distinguish the two ends. Diameter,
height and cut-depth edits, projection extrusion suppression, undo and container
reopening preserve the cut cap's source. These bring the suite to 163 examples.

The `partial-cylinder-wall-join` example joins one end of a rectangle outside a
projected-size circle to a cylindrical neck on a beveled base. Its shared curved
wall covers only part of the neck height. Analytic expectations add the wing
area `2 a L - a sqrt(r²-a²) - r² asin(a/r)` times its depth to the existing
cylinder/frustum volume. Depth edits, suppression, undo and reopening are checked.

The `partial-cylinder-cap-pocket` design attaches a pocket to the merged top of
the neck and wing. Its volume removes π mm³ (radius 2 mm, depth 0.25 mm). The
pocket follows base-height edits, and remains on the neck when the wing is
suppressed; the saved attachment follows the resulting face, not a frozen plane.

The `partial-cylinder-tip-fillet` design rounds the wing's two free tip edges.
Each removes `width × radius² × (1 − π/4)` from the joined body. Radius and
base-height edits, suppression, undo and saved-history reopening are checked.
The I/O incidence regression also checks this small removed volume using the
same fine chord tolerance before and after: adaptive whole-body measurements
change resolution when a smaller radius is introduced.

The `partial-cylinder-ring-neck-fillet` and `partial-cylinder-ring-base-fillet`
examples round the concave and convex circular shoulders independently. Expected
volumes integrate πr(z)² along the analytic cylinder/cone profile, replacing the
corner with a tangent radius-1 mm arc. Radius-1.5 mm edits, source-height edits,
suppression, undo and reopening preserve that history. An I/O regression measures
before/after geometry at a fixed 0.0001 mm chord tolerance and checks the small
added/removed volume within 0.02 mm³. There are 168 examples.


The `partial-cylinder-wing-vertical-fillet` and `partial-cylinder-wing-bottom-fillet`
examples round the concave wing roots. The vertical round adds the area between
its tangent circles and straight wing wall, integrated by Green's theorem, times
the wing depth. The lower round adds a torus segment clipped by the two planar
wing sides; its expected volume integrates circular-strip areas through height.
The ordinary I/O test checks both radii (1 and 1.5 mm) against these independent
references to 0.02 mm³, and refuses radii that cross the shoulder or consume the
wing tip. The saved recipes also check radius/source-height edits, suppression,
undo and reopening. There are 170 original examples.

The `partial-cylinder-combined-corner-fillet` and
`partial-cylinder-combined-all-fillet` examples round both vertical wing roots
and their lower circular edge together. The second also rounds both separate
shoulder rings in the same feature. Their expected added volume includes the
intersection of the vertical rounds and the lower torus, rather than adding the
independent edge volumes. The I/O regression integrates that clipped torus and
checks radii 1 and 1.5 mm within 0.02 mm³ at a common 0.000025 mm chord tolerance.
The shoulder contributions use the analytic cylinder/cone integrals above.
Reversing the corner edge order preserves the result. Both saved designs also
check radius/source-height edits, suppression, undo and reopening. There are
172 original examples.

`partial-cylinder-touching-tip-fillets` uses a 2 mm deep wing with 1 mm rounds
on its two tip edges. The rounds meet on one cylindrical carrier; the kernel
retains a seam between their two faces. Removing `2 × 3.75 × r² × (1 − π/4)`
from the unrounded wing gives the analytic volume. A radius edit to 0.75 mm
separates the carriers again. Radius, source-height, suppression, undo and
reopening checks accompany an I/O regression that requires exact face lineage
before coalescing the two faces.

The two `paired-chamfer-projection-*` examples bevel both ends of one cylinder,
project either the upper or lower beveled circle, and extrude the live projection
as a separate body. For cylinder radius R, height H, equal bevel setback d and
projected length L, the analytic total volume is
`π × (R²H − 2d²(R − d/3) + (R − d)²L)`. The two removed rings are symmetric
about H/2; the projected cylinder supplies the remaining first moment. Diameter,
height, bevel and projection-length edits, suppression, undo and save/reopen are
checked. The split-cylinder example below extends this collection.


`split-cylinder-cap-fillet` joins a centered 6 mm band of a radius-10 mm disk
above an 8 mm cylinder, then rounds both concave shoulders. Its original
SolidCraft recipe checks save/reopen, radius, diameter, base-height and grip-height
edits, plus suppression. The unrounded volume is `pi R² h + A H`, where
`A = 2 (a sqrt(R²-a²) + R² asin(a/R))` and `a=3 mm`. The two rounds add
`4 integral[0,r] sqrt(R²-(a+s)²) g(s) ds`, with
`g(s) = r - sqrt(r²-(r-s)²)`. Integrating the same slice times `h+g(s)/2`
gives the added first moment for the centroid check. These expectations come
from geometry, not sampled kernel output. The I/O regression also compares
rounded and unrounded volumes at the same chord tolerance, so cylinder
meshing error cannot conceal a missing round.


`split-cylinder-consumed-wall` uses the same independent integral at a 5 mm
grip height, with the round radius driven by that height. The planar walls are
fully consumed, leaving each round tangent to the upper cap. Height edits preserve
that relationship; reducing the radius separately restores the planar walls.
The I/O regression also refuses a radius larger than the wall height and verifies
that the limiting result has no crossed face loops.

`split-cylinder-grip-corner-fillets` rounds the four vertical corners of the
same clipped cylindrical grip. With `a=3`, the center of each tangent round is
`(a-r, sqrt((R-r)²-(a-r)²)) = (cx, cy)` and its cylinder contact has
`tx=R cx/(R-r)`. Let `F(R,x)=(x sqrt(R²-x²)+R² asin(x/R))/2`.
The removed volume is
`4 H [F(R,a)-F(R,tx)-(a-tx)cy-F(r,r)+F(r,tx-cx)]`.
Its centroid follows from the base cylinder and the remaining constant-section
grip. Radius, diameter and both heights are edited, saved, reopened and undone;
suppressing the rounds restores the independently calculated unrounded volume.
