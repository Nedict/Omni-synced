# SolidCraft C# interop (ModelForge workspace drives Rust)

The ModelForge C# workspace was written against the SolidWorks COM interop
(`ModelForge.Interop.sldworks`). This directory holds the replacement that lets **that same C#**
drive a SolidCraft (Rust) session in-process instead.

Two pieces:

- `crates/ffi` — the Rust `cdylib` (`solidcraft-ffi`): a **typed C ABI** over
  `solidcraft_engine::Session`. Opaque handles, `#[repr(C)]` structs, explicit error codes.
  No JSON on the hot path.
- `csharp/SolidCraft.Interop` — the C# side: P/Invoke bindings that mirror the Rust types
  one-for-one, plus the `ModelForge.Interop.sldworks` / `swconst` compatibility assemblies the
  workspace compiles against.

## Type-safety contract

The boundary is typed, not stringly. Each concept has exactly one definition, mirrored:

| Concept | Rust | C# |
|---|---|---|
| session handle | `SessionHandle` (opaque) | `SafeSessionHandle` |
| status | `ScStatus` (`#[repr(C)]` enum → `int32`) | `ScStatus` enum |
| 3-vector | `ScVec3 { x, y, z: f64 }` | `ScVec3` struct |
| point/plane frame | `ScFrame` | `ScFrame` |
| entity reference | `ScEntityRef { kind, doc, id }` | `ScEntityRef` |
| body/face/edge ids | `u64` | `ulong` |

The **layout of every shared type is asserted on the Rust side** (`const _: () = assert!(...)`
size/offset checks) and re-checked at runtime once per process (`sldcraft_layout_guard`), so a
mismatch fails loudly at startup instead of corrupting memory.

Bindings are kept in sync by generation: `tools/gen-ffi.py` reads the Rust header
(`crates/ffi/include/solidcraft_ffi.h`, produced by `cbindgen`-style export) and emits the C#
`[StructLayout]` declarations, so the two definitions cannot drift.

## Building

```sh
cargo build -p solidcraft-ffi            # target/debug/libsolidcraft_ffi.dylib
cargo xtask csharp                       # build the interop + run the C# conformance tests
```
