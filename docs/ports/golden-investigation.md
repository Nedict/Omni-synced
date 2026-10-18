# Golden test investigation: robustness_goldens seed 13 and seed 67 on Linux aarch64

Investigation of the two failing seeds found during the ARM64 port (2026-10-18).
Branch `port/arm64`. No golden baseline, tolerance, or expected value was changed.

## Reproduction

- Host: Windows ARM64; Linux aarch64 via WSL Ubuntu 22.04 (native, not emulated).
- Command: `cargo test -p solidcraft-engine --lib robustness_goldens::seed_13` (and seed_67).
- x86_64 machine: not available locally. The stored golden values are the reference
  (generated on the Linux x86_64 runners that run these seeds on every push to main).

## Step sequences (from goldens.json)

- seed 13: `revolved section, cut cylinder, join box, rectangular pattern, cut cylinder, cut box, fillet, hole`
  - golden (x86_64): bodies=1 volume=2396.939546 area=2101.521015 faces=41 edges=103 shells=1
  - aarch64:        bodies=1 volume=2410.783741 area=2112.846954 faces=39 edges=99  shells=1
- seed 67: `extruded profile, cut cylinder, hole, join cylinder, join box, hole, chamfer, rectangular pattern`
  - golden (x86_64): bodies=2 volume=27607.962186 area=7943.46222  faces=37 edges=88  shells=2
  - aarch64:        bodies=2 volume=27607.962204 area=7943.462219 faces=36 edges=86  shells=2

## Step-by-step on aarch64 (Linux)

Seed 13:
```
step 0 revolved section     bodies=1 vol=2061.632444 area=1112.093468 faces=10 edges=24 shells=1
step 1 cut cylinder         bodies=1 vol=2061.632444 area=1112.093468 faces=10 edges=24 shells=1   (cut missed)
step 2 join box             bodies=1 vol=2530.338450 area=1605.436123 faces=20 edges=45 shells=1
step 3 rectangular pattern  bodies=1 vol=2999.017236 area=2088.958784 faces=28 edges=64 shells=1
step 4 cut cylinder         bodies=1 vol=2694.447620 area=2005.864711 faces=35 edges=84 shells=1
step 5 cut box              bodies=1 vol=2694.447620 area=2005.864711 faces=35 edges=84 shells=1   (cut missed)
step 6 fillet               bodies=1 vol=2688.990547 area=1999.210135 faces=36 edges=87 shells=1
step 7 hole                 bodies=1 vol=2410.783741 area=2112.846954 faces=39 edges=99  shells=1   (final)
```

Seed 67:
```
step 0 extruded profile     bodies=1 vol=24750.081338 area=5897.135235 faces=10 edges=24 shells=1
step 1 cut cylinder         bodies=1 vol=24607.091026 area=5910.634139 faces=14 edges=35 shells=1
step 2 hole                 bodies=1 vol=24112.674497 area=6183.994842 faces=16 edges=41 shells=1
step 3 join cylinder        bodies=1 vol=24664.832021 area=6478.509078 faces=19 edges=47 shells=1
step 4 join box             bodies=2 vol=27789.832021 area=7791.009078 faces=25 edges=59 shells=2
step 5 hole                 bodies=2 vol=27632.778480 area=7948.082187 faces=28 edges=65 shells=2
step 6 chamfer              bodies=2 vol=27627.992916 area=7937.051362 faces=32 edges=74 shells=2
step 7 rectangular pattern  bodies=2 vol=27607.962204 area=7943.462219 faces=36 edges=86 shells=2   (final)
```

## Where it diverges

The divergence is topological (face/edge counts), not just floating-point rounding beyond the 1e-6 tolerance: seed 13 loses 2 faces and 4 edges (and +0.6% volume), seed 67 loses 1 face and 2 edges. The most likely last-diverging operation is the final feature of each (seed 13: `hole`; seed 67: `rectangular pattern`), which perform booleans/patterns whose merge decisions depend on tolerance comparisons of near-coincident geometry.

I could not obtain an x86_64 step-by-step trace (no x86_64 toolchain or machine available here), so I cannot state with certainty which step first differs from x86_64. The stored golden final values are used as the reference.

## Checks performed

- FMA / fused multiply-add: no explicit `f64::mul_add` or `fma` usage anywhere in `crates/`. Rust compiles without `-ffast-math`, so the compiler does not auto-contract. Ruled out as the direct cause, but aarch64's default FP rounding/operation ordering in the kernel's numeric code cannot be fully excluded without an x86_64 trace.
- Hash/iteration order: `HashMap`/`HashSet` are used in `crates/kernel/src/distance.rs` and `crates/kernel/src/tess/*` (tessellation/display paths), not in the B-rep topology path that produces face/edge counts. These affect display mesh, not the measured B-rep. Ruled out for topology divergence.
- Tolerance constants: many in `crates/kernel/src/distance.rs` and the boolean/split code. The differences (0.5-0.6% volume, 2-4 edges) are larger than 1e-6 relative, so this is not a sub-epsilon rounding flip in a single scalar; it is a topological decision (a face/edge that is or is not merged) sitting near a tolerance boundary.
- `usize` width: both targets are 64-bit; no platform-sized value issue.

## Classification

(c) tolerance-boundary sensitivity: a boolean/pattern merge decision sits close enough to a tolerance that the (small) floating-point differences between aarch64 and x86_64 flip it, changing the resulting topology. It is not (b) order-dependence (no HashMap iteration on the topology path) and not obviously (d) a kernel bug that would reproduce on x86_64, since these seeds pass on the x86_64 CI runners. (a) cannot be fully excluded because the exact first-diverging step and its numeric distance to the threshold were not measured on x86_64.

## Recommendation

Do not loosen the 1e-6 tolerance and do not bless the golden values blindly: that would mask a real (if small) behavioral difference and would also overwrite the x86_64 baseline via the shared `seeds` table. Instead:

1. Add architecture to the platform key in the golden framework so aarch64 Linux can carry its own verified values without affecting x86_64 Linux. See `docs/ports/proposed-arch-override.patch`.
2. Investigate the specific boolean/pattern operation in seeds 13 and 67 with an x86_64 reference run (or `ROBUSTNESS_GOLDENS_BLESS` on aarch64 after reviewing the diff) to decide whether the tolerance constant should move. The tolerance change, if any, must be justified by evidence and approved before application.

## What I could not verify

- No x86_64 toolchain or machine is available in this environment, so I could not capture the x86_64 step-by-step trace or confirm which step first diverges. The stored golden final values are used as the reference.
- The exact tolerance distance (got vs threshold) for the first differing operation was not measured; it requires an x86_64 run or instrumenting the kernel's boolean merge predicates.
- Whether seeds 13/67 also fail on Windows ARM64 was not tested (no device).