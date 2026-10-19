# Upstream golden history for robustness seeds 13 and 67

Verified on 2026-10-18 against upstream before rebasing `port/arm64`. The
expected values the rebased branch carries must (and do) come from upstream
`main`, not from the fork.

## Refs

- Upstream remote: `https://github.com/tangency-labs/solidcraft.git` (added as
  `upstream`), then `git fetch upstream --tags`.
- `upstream/main` HEAD at fetch: `e4ff62c825feebd1e1515cb94dca94415498a72d`
  ("CI: run the triage and claim bots on the trusted light runner (#2655)",
  2026-10-18T13:10:23-04:00).
- Merge base with `port/arm64` (fork tip `3fb67a30`):
  `b685b7301f35948ab75015d48ec8b89cfd8252b7` ("Preferences: reverse horizontal
  pan direction (#2007) (#2047)", 2026-10-18T03:12:23-04:00). `port/arm64` was
  5 commits ahead, 39 behind.

## Commits on upstream/main that re-record seeds 13 and 67

`git log --first-parent upstream/main -- tests/robustness/goldens.json` shows
exactly two commits since the merge base; both are on `upstream/main`
(verified with `git merge-base --is-ancestor <commit> upstream/main`, exit 0):

| commit | subject | seed 13 | seed 67 |
|---|---|---|---|
| `72b7fc385b6fda5cc97f711e14624d8e4ce95810` | Fix red main: record robustness seeds 13, 67 and 94 after #2064 (#2396) | faces 41 → 39, edges 103 → 99, area 2101.521015 → 2112.846954, volume 2396.939546 → 2410.783741 | faces 37 → 36, edges 88 → 86, area 7943.46222 → 7943.462219, volume 27607.962186 → 27607.962204 |
| `f5a868efda43d37458cfeb62a063a868c56f6e68` | Fix red main after #2247: record its robustness seeds, and keep older designs' joins of split pieces working (#2540) | unchanged | faces 36 → 35, edges 86 → 84, bodies 2 → 1, rejected 0 → 1, unsupported 2 → 1, area 7943.462219 → 7940.064, volume 27607.962204 → 27606.450855 |

The re-records named in the task are confirmed. Each moved values the task list
did not mention: `72b7fc38` also moved seed 13's area and volume, and seed 67's
area and volume (in the last digits); `f5a868ef` also moved seed 67's body,
rejected and unsupported counts.

- `72b7fc38` (measured on Linux) records seeds 13, 67 and 94 because #2064
  (multi-profile extrusion) merges coplanar faces the Boolean used to leave
  split. Its note: seed 67 has one face/two edges fewer from its first cut on
  at the same volume at every step; seed 13's second cylinder cut leaves
  35 faces/84 edges instead of 37/88, so its random fillet (edges picked by
  index) rounds different edges. Every step still passes the validity and
  volume-direction checks.
- `f5a868ef` records 16 seeds (7, 9, 12, 16, 18, 22, 27, 31, 33, 45, 50, 54,
  67, 72, 81, 93), recorded on Linux, because #2247 stops a cut from splitting
  a body's separate pieces into bodies of their own (`bodies N → 1`), after
  which the generator can build different steps.

## Current stored values at `e4ff62c8`

Shared records (used on Linux):

- seed 13: faces 39, edges 99, shells 1, bodies 1, rejected 1,
  unsupported 0, area 2112.846954, volume 2410.783741, outcome ok.
- seed 67: faces 35, edges 84, shells 2, bodies 1, rejected 1,
  unsupported 1, area 7940.064, volume 27606.450855, outcome ok.

Platform overrides at `e4ff62c8` live under `platforms.windows.seeds` for seeds
1, 23, 34, 67 and 94. Seed 13 has no override; seed 67's Windows override is
faces 36, edges 86, shells 2, bodies 2, area 7943.46222, volume 27607.962186
(the value #2396 recorded and #2540 did not touch). Linux uses the shared
records for both seeds.

## The fork's stored values before the rebase

`port/arm64` (tip `3fb67a30`) recorded seed 13 as faces 41, edges 103,
area 2101.521015, volume 2396.939546 and seed 67 as faces 37, edges 88,
area 7943.46222, volume 27607.962186. Those are the pre-#2064 values: the
state of `upstream/main` before `72b7fc38` (seed 67's area and volume also
equal its Windows override, which #2540 did not change).

## After the rebase

`git diff upstream/main -- tests/robustness/goldens.json` on
`port/arm64-rebased` is empty: the branch carries upstream's records
unchanged. The Linux results of the rebased branch are in
`rebase-verification.md`.
