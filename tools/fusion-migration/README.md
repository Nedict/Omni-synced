# Fusion migration

Bring work out of Autodesk Fusion and into SolidCraft, with the result checked rather than assumed.

A `.f3d` cannot be read directly, and not for the reason it first appears: its ZIP entries use
Zstandard (method 93), which the `zip` crate's deflate-only build refuses, so a `.f3d` is reported
as an invalid archive entry. Its solids are Autodesk ASM (`.smb`), and its feature history lives in
GUID-keyed binary streams. Everything here therefore goes through Fusion itself, using its public
scripting API — nothing is decompiled, and nothing in this directory came from inspecting Autodesk
binaries. Geometry leaves as STEP; the parametric recipe leaves as JSON.

Two scripts run inside Fusion, three run here.

## In Fusion

Copy a folder from `fusion/` into Fusion's script folder, then run it from
**Utilities → ADD-INS → Scripts and Add-Ins**. Both are resumable: stop them and run them again
and they skip what is already written.

| Script | Writes | What it is for |
|---|---|---|
| `ExportAllDesigns` | `~/cad/fusion-export/**.step` + `.f3d`, and `manifest.json` | The geometry, plus **Fusion's own volume and area per design** |
| `ExtractTimeline` | `~/cad/fusion-timeline/**.timeline.json` | The parametric recipe: parameters, sketches, features |

`manifest.json` is the point of the first script. Exporting is easy; knowing the export is
*correct* is not, and a migration that quietly loses a pocket is worse than one that fails.

`ExtractTimeline` records each feature's driving **expression** (`width/2 + 3 mm`), not just its
resolved number, because the expression is what keeps a rebuilt design parametric. It also records
a point lying **on** each referenced edge and face, and a point inside each profile. Fusion's
internal edge ids mean nothing here, but a point on an edge is geometry, and geometry travels —
that is what lets a fillet or a shell be re-created instead of merely reported as lost.

Reading the model through the API also sidesteps the Y-up/Z-up problem: Fusion's STEP export is
Y-up and SolidCraft is Z-up, but model coordinates come out of the API Z-up already.

## Here

```sh
python3 tools/fusion-migration/migrate.py --convert --thumbs
python3 tools/fusion-migration/timeline_to_solidcraft.py ~/cad/fusion-timeline --all --verify
python3 tools/fusion-migration/f3d_inventory.py --open
python3 tools/fusion-migration/golden/run_golden.py
```

- **`migrate.py`** imports every exported STEP and compares it against Fusion's recorded volume.
  Exit status is non-zero if any design fails, so it can gate a migration. The tolerance (1e-3
  relative) sits above a measured floor, not a guessed one: volume on curved faces is measured from
  a tessellation, and a plain cylinder reads 1.1e-4 under its analytic volume.
- **`timeline_to_solidcraft.py`** turns a timeline into a command script that rebuilds the part.
  Every step is either translated or reported as untranslated **with a reason** — a half-migrated
  part that claims to be whole is the one failure mode worth engineering against.
- **`f3d_inventory.py`** builds a visual contact sheet of an archive without opening Fusion, by
  decompressing the previews Fusion stores inside each `.f3d` (needs `pip install zstandard`;
  without it the rest still runs). Useful for deciding which parts deserve a rebuild at all.
- **`golden/run_golden.py`** is the test suite, and it needs no Fusion.

## Measured on a real hub

270 designs, 2,797 timeline steps:

| | |
|---|---|
| Designs with no modellable history | 106 — imported bodies and assemblies, where the STEP export is the right answer |
| Designs with history | 164 |
| Coverage, all timeline steps | 82% |
| Coverage, modelling steps only | 91% |
| Designs translated in full | 49 |
| Of those, building without error | 20 |

Both coverage numbers are shown because the difference is 259 steps that are not modelling
operations at all — assembly placements, snapshots, canvases, and construction planes that
sketches no longer need now they rebuild from their own frame. Quoting only the larger number
would be moving work out of the denominator.

Of 136 designs that do not build, **84 fail inside the kernel rather than in translation**:
fillets on curved edges (`only straight edges can be blended`, `both faces next to the edge must
be planar`), boolean fallbacks where bodies meet, contact limits, and shell cavity ownership.
Those are reproducible failures on real geometry and belong in issues, not in workarounds here.

Note the trade this represents. Before fillet edges could be recovered, more designs *built* —
because their fillets were being silently skipped. A part that quietly omits its rounds is worse
than one that fails loudly, so the number that matters is designs translated in full (49, from 15).

## What translates today

| Fusion | SolidCraft | Notes |
|---|---|---|
| User parameters | `parameters.add` | expressions preserved |
| Sketch on an origin plane | `sketch.create` + curves | lines, circles, arcs, splines |
| `ExtrudeFeature` | `solid.extrude` | distance, taper, two-sided, symmetric, operation |
| `FilletFeature` | `solid.fillet` | constant radius, by edge points |
| `ChamferFeature` | `solid.chamfer` | equal distance |
| `ShellFeature` | `solid.shell` | by removed-face points |
| `DraftFeature` | `solid.draft` | neutral plane assumed XY |
| `RevolveFeature` | `solid.revolve` | axis guessed; check it |

Not yet mapped: lofts, sweeps, holes, patterns, mirrors, combines, sheet metal, sketches attached
to faces or offset planes, and ellipses. These are reported per step, so coverage is a number you
can read rather than a claim.

## Tests

`golden/run_golden.py` hand-authors a timeline in exactly the shape `ExtractTimeline` emits, for
parts whose volume is known in closed form, then translates, builds and compares:

```
case                       expected          built      drift  result
RoundedPlate             28542.4778     28542.3089   5.92e-06  pass
ChamferedBlock           29640.0000     29640.0000   0.00e+00  pass
ShelledBox               11712.0000     11712.0000   0.00e+00  pass
ParametricPlate          10000.0000     10000.0000   0.00e+00  pass
NegativeControls          6000.0000      6000.0000   0.00e+00  pass

parametric rebuild check
  thk 8mm -> 20mm: expected 25000.0, got 25000.0000  pass
```

The last check is the one that matters: a rebuilt part must still rebuild when a parameter changes,
or this was an expensive way to produce a frozen solid.

`NegativeControls` is deliberately unmigratable in part — it holds a feature type with no mapping
and a suppressed feature — and the suite **fails if either comes back translated**. As in
`tools/oracle-analytic`, never move an expected value to match what SolidCraft produced: if they
disagree, find out which one is wrong.

Scripts are plain Python 3 on the standard library, except the optional `zstandard` for previews.
