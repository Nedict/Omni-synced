# ModelForge / SolidWorks interop (C# drives Rust)

The ModelForge C# workspace was written against the SolidWorks COM interop
(`ModelForge.Interop.sldworks`). SolidCraft provides a replacement so **that same C# drives a
SolidCraft (Rust) session in-process** instead. This document records what is implemented, how it
is verified, and what remains.

## Architecture

| Layer | Where | What |
|---|---|---|
| C ABI | `crates/ffi` (`solidcraft-ffi`) | typed `extern "C"` over `solidcraft_engine::Session`; `#[repr(C)]` shared types |
| C# typed bridge | `csharp/SolidCraft.Interop/Interop` | P/Invoke mirror of the ABI (`ScVec3`, `ScEntityRef`, …), layout-checked at startup |
| SolidWorks interfaces | `csharp/SolidCraft.Interop/sldworks` | `ModelForge.Interop.sldworks` re-declared and implemented over the bridge |
| SolidWorks constants | `csharp/SolidCraft.Interop/swconst` | 626 enums, 5477 constants, generated from the ModelForge swconst assembly |

The workspace C# is **unchanged**: it still does `using ModelForge.Interop.sldworks;` and calls the
same members. Only the implementation behind those interfaces changes — from COM/SolidWorks to
SolidCraft.

## Type safety at the boundary

The shared types are defined once in Rust and mirrored in C#:

- each `#[repr(C)]` type has compile-time size assertions on the Rust side
  (`crates/ffi/src/types.rs`);
- the C# side checks the same sizes at startup through `sldcraft_layout_guard`, which returns the
  offending type's name on a mismatch — so a drift stops the process instead of corrupting memory;
- units convert only at the interface layer (SolidWorks works in metres, SolidCraft in mm/degrees).

## Verification

`cargo xtask csharp` builds the cdylib and the bridge and runs
`csharp/SolidCraft.Conformance`, which loads the real library and asserts:

- the shared type layouts agree with Rust, and the guard catches a deliberate mismatch;
- the part workflow (rectangle → extrude → through cut) matches analytic volumes;
- hostile input (NaN, unknown commands, missing parameters) is rejected, never crashes;
- a workflow written **exactly the way the workspace writes it** — `ISldWorks.NewPart`,
  `Extension.SelectByID2`, `SketchManager.InsertSketch`/`CreateCornerRectangle`,
  `FeatureManager.FeatureExtrusion2`, `EditRebuild3`, `Extension.CreateMassProperty`,
  `IPartDoc.GetBodies2`, and persist references through
  `GetPersistReference3`/`GetObjectByPersistReference3`/`IsSamePersistentID` — against the Rust
  session.

### Whole-workspace compile check

The strongest compatibility evidence: every `.cs` file of the ModelForge workspace build
(`workspace-builds/64166479807f`, 50 files) compiles against the interop assembly. Measured on
2026-10-12:

```
total errors:                       140
of which WinForms / System.Drawing: 140
SolidWorks-API errors:                0
```

Every remaining error is the workspace's own **WinForms GUI** (Windows-only, unrelated to driving
SolidCraft). The SolidWorks API surface it uses — `ISldWorks`, `IModelDoc2`, `IPartDoc`,
`IAssemblyDoc`, `IComponent2`, `IFeature`, `IFeatureManager`, `ISketchManager`, `ISelectionMgr`,
`IMassProperty`, `IBody2`, `IFace2`, `IEdge`, `ISketch`, `IMate2`, `IModelDocExtension`, the math
types and the feature-data interfaces — has no compile errors at all.

## Status of the implementations

Implemented over SolidCraft commands and typed queries: documents (new/open/save/close), sketches
(corner rectangle, line, circle), the feature manager (extrude, cut, fillet, chamfer, loft),
mass properties, the document object model (features, bodies, faces, edges, sketches, sketch
segments and points), selection, configurations, persist references, materials, math types,
drawings/views/sheets, assemblies (components, mates, replace) and notes.

Members that are declared but not yet backed by an engine behaviour return an empty or neutral
value (`NotSupportedException` is never thrown from a command path); the conformance workflows
above cover the part-modelling path end to end. Regenerating the constants and re-running
`cargo xtask csharp` is the check that keeps this honest.

## Regenerating the constants

```sh
python3 tools/gen-swconst.py --dll <ModelForge.Interop.swconst.dll> --out csharp/SolidCraft.Interop/swconst
```

The values are read from the assembly's .NET metadata, never transcribed by hand.
