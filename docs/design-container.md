# Design containers, version 1

New designs use `.solidcraft`; `.solvecraft` remains a supported legacy extension. Opening detects the ZIP
local-header signature (`50 4b 03 04`) independently of the name; otherwise it uses
the existing JSON design reader and migrations. Plain JSON remains a fully supported,
self-contained input/output format. `io::write_design` keeps its JSON contract;
`io::write_design_container` is the fallible container writer.

Packages have `container.json` with `{"format":"solidcraft-container/1"}` and
`document.json` with the versioned parametric document (currently `solidcraft/15`).
The independent versions let document migrations evolve without changing blob encodings.
Existing serialized document fields are retained automatically, without a second copy
of the document schema or a deep clone of embedded payloads.

Within `document.json`:

- Import features have empty `step` and a `step_blob` reference.
- Mesh imports retain each mesh's name and optional colour, with empty `positions`
  and `triangles` plus a `mesh_blob` reference.
- Canvases retain placement, format and pixel metadata, with empty `data` plus an
  `image_blob` reference. Raw image bytes replace base64 in the package.

References are `blobs/<lowercase SHA-256>.<step|mesh|png|jpeg>`, hashed over the exact
uncompressed payload. Identical content with the same encoding is written once.
Required parts, malformed references, ambiguous embedded payloads, incorrect hashes,
invalid mesh data and unsupported versions produce errors before session adoption.

## Mesh encoding

Mesh blobs begin with the eight ASCII bytes `SCMESH01`, followed by little-endian
`u64` coordinate count and `u64` index count. These count scalar values, not vertices
or triangles. Next come that many IEEE-754 `f64` coordinates, then `u32` indices.
Both counts must be divisible by three; the byte length must match exactly. Coordinates
must be finite and at most 1e9 mm in magnitude; indices must address an existing vertex.
Names and colours remain in JSON. Signed zero and full coordinate precision survive.

The default `fast` policy stores STEP and binary meshes without Deflate for faster
saving and reading. Small document metadata uses Deflate level one; PNG/JPEG are
stored directly. An explicit `compact` policy compresses STEP and mesh payloads at
level one. Readers support both policies. This deliberately favors interactive file
operations over maximum compression; compressed STEP was slower in the storage benchmark.
Use `"compression":"compact"` with `file.save` or `file.save_as` to favor file size,
or `io::write_design_container_with_compression` from Rust.

## Bounds and recovery

Input and individual expanded parts are capped at 256 MiB. Total declared expanded
parts and resolved payloads are each capped at 512 MiB, including repeated references
in the latter budget. There are at most 10,000 entries. Duplicate/ambiguous entry names,
path traversal, ZIP64 and multi-disk packages are refused. Entries are read in memory;
no archive paths are extracted to the filesystem. Exact part lengths and ZIP CRCs are
checked, along with blob hashes. Hashes detect correspondence and corruption, not trust.

Saving retains the existing temporary-file/flush/rename protocol and `.bak` backup.
Recovery reads both older JSON and new containers. Browser OPFS, downloads and recovery
use the same container reader/writer; JSON export and Save As remain available.

## Provisional saved display

Experimental engine saves with `preview: true` can include `cache/preview.json` and
`cache/preview.bin` when the evaluated
world display meshes are already warm. Ordinary saves omit this redundant display data
to avoid extra save work; it is not the proposed evaluated B-rep cache. Saving never triangulates a cold body for this
cache. Pending occurrence moves, active sketch edits, feature errors and oversized
previews omit it. The cache is bounded to 8 MiB, 1,024 bodies and 100,000 triangles;
its manifest is at most 64 KiB. Body names/colours and contiguous binary mesh slices
use the same lossless encoding as imported meshes. Extra cache size is optional;
it is omitted if it would put the container near the file-size cap.

The manifest includes SHA-256 of the exact document metadata (whose blob references
contain content hashes), the preview payload, `solidcraft-preview/1` and the application
package version. A version mismatch invalidates the display cache. Development builds
with the same package version may briefly show an older picture before the authoritative
rebuild replaces it; previews never supply model state. Earlier draft containers using
`solvecraft-container/1` remain readable, with their obsolete previews treated as misses.
Missing, obsolete, oversized or malformed preview data is a cache miss, not a failed
design open. All archive-wide safety budgets still apply.

Desktop Open can render these meshes on its existing file worker and show a clearly
labelled saved picture in the opening dialog before timeline reconstruction finishes.
It uses only the normal command's file snapshot, without a second file read. The picture
is approximate shading without face paint or picking. A conservative 16-million-pixel
raster budget rejects excessive overdraw before rendering, including repeated large
triangles in a small hostile cache. The picture is never adopted as editable
geometry, used for measurements, or used to seed evaluated model state. The original
design remains intact until the complete rebuilt session is ready. Cancellation or
any change of active document/revision discards both preview and completion.

Normal design reading, CLI and browser opening still rebuild authoritatively; no
evaluated B-rep is deserialized or adopted. Portable evaluated B-rep/state restoration
and interactive cached viewport navigation remain further work in #303.
