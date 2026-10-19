# Rebase verification: port/arm64-rebased

Verifies that the ARM64 port branch, rebased onto upstream main, passes the robustness goldens on Linux x86_64 and Linux aarch64 against upstream's current stored values. No expected values, tolerances or goldens were changed.

## Commits and runs

- Upstream base: tangency-labs/solidcraft main at e4ff62c8
- Verified code commit: 6783c966. The only later change before the cleanup, 8e7e82d0, added .github/workflows/rebased-verify.yml, which was removed in a23a5cb2.
- Verification run: https://github.com/varunkumar898/solidcraft/actions/runs/39152103110
  - robustness_goldens (x86_64): success (55m13s)
  - robustness_goldens (aarch64): success (44m40s)
- Observation-only run (diagnostic; goldens rewritten in CI; not a pass/fail check): https://github.com/varunkumar898/solidcraft/actions/runs/39209160787

## Results

Upstream stored values, from tests/robustness/goldens.json at e4ff62c8:

| Seed | Upstream | x86_64 CI | aarch64 CI | aarch64 WSL | Windows ARM64 |
|------|----------|-----------|------------|-------------|---------------|
| 13 | faces 39, edges 99, volume 2410.783741, area 2112.846954 | pass | pass | not captured | unresolved: 40 faces / 101 edges |
| 67 | faces 35, edges 84, volume 27606.450855, area 7940.064 | pass | pass | not captured | not captured |

Observed aarch64 values from the observation run match upstream for both seeds (seed 13: 39/99; seed 67: 35/84, unsupported 1).

## Not verified

- aarch64 WSL row: not captured in this document.
- Windows ARM64 seed 13: builds 40 faces / 101 edges against upstream's 39/99. The divergence is inside the untraced prism/coplanar boolean path. Tracked separately in docs/ports/golden-trace-report.md. Not addressed by this branch.
- Windows ARM64 seed 67: not measured against the current record.
- Windows x86_64: not measured.
- MSVC ARM64 MSI build and install: not run.

## Notes

- The stale goldens that made CI red earlier were one upstream re-record behind. This branch does not change them.
- No arch-scoped or Windows-only override is added. The proposal is in docs/ports/proposed-arch-override.patch and is not applied.
