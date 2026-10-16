# Neutral (CadIr) import

Fusion `.f3d` / `.f3z` and future cadmpeg decodes persist **canonical CadIr JSON** instead of
re-stated STEP. Container packages store it as `blobs/<sha256>.cadir` (see
[design-container.md](design-container.md)); the Import feature keeps an empty `step` and the
inline or blob-backed `cadir` field.

On eval, `solidcraft-cadmpeg-transfer` builds kernel bodies from the IR (v1 uses an in-memory
STEP bridge, then portable B-rep → truck). STEP-only imports (`cadir` empty) behave as before.

Blob hashes are natural **tier keys** for CollabCraft / `craft-tier-store` cold storage; warm
tiers may hold derived native B-rep cache without changing the authoritative CadIr blob.

See also [step-import.md](step-import.md) and [native-brep.md](native-brep.md).
