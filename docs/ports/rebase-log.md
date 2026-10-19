# Rebase of port/arm64 onto upstream main (port/arm64-rebased)

Command: `git checkout -b port/arm64-rebased port/arm64`, then
`git rebase upstream/main`.

- `port/arm64` before: `3fb67a30cd8aa5023bee3e510bb1cd3dc2e139d7` (base
  `b685b730`).
- `upstream/main`: `e4ff62c825feebd1e1515cb94dca94415498a72d`.
- `port/arm64-rebased` after the rebase:
  `4728e889aca97a7380928013442afeee372af221`.

## Replayed commits

| port/arm64 | port/arm64-rebased | subject | result |
|---|---|---|---|
| `f3305970eccda099a88aff67da29a9f32102f613` | `4e438e340c054f47aaff16266ca176a6c598f46a` | arm64: port status and audit docs | clean |
| `93abad9c8f43a0b5f9e776565bb1bf5432cb749c` | `b7b320ecececcf5812e7c9b9a7951a96148c7f4e` | windows-arm64: port scaffold doc | clean |
| `af420f4a091e139d688b940ce4266bea944bfd5f` | `5843c31cbfaaee1ac429bc1599d65b4af1efc278` | ci: native Linux ARM64 compile gate (ubuntu-22.04-arm) | 1 conflict, resolved |
| `4e4c7b7748a7bd98cb5464097dd5e3c669dd4a09` | `1e9fadc70a5bd0305e4cf680c4e4b0bf39143ae3` | arm64: record verified build/run results and known gaps | clean |
| `3fb67a30cd8aa5023bee3e510bb1cd3dc2e139d7` | `4728e889aca97a7380928013442afeee372af221` | arm64: investigate robustness_goldens seed 13/67 divergence on aarch64 | clean |

## Conflicts resolved (one)

### `.github/workflows/ci.yml` (replaying `af420f4a`)

- Cause: the port commit edits the `ci` job's `needs:` line to add
  `linux-aarch64-gates`. Since the merge base, upstream gave the same `ci` job
  an `if:` that skips draft pull requests — in the two lines the port hunk
  touches — so the two changes overlapped.
- Upstream side (at `e4ff62c8`):
  `needs: [changes, checks, clippy, build-tests, tests, doc-tests, oracle, windows-fxc, windows-startup]`
  and
  `if: always() && (github.event_name != 'pull_request' || !github.event.pull_request.draft)`.
- Port side: the same `needs` list plus `linux-aarch64-gates`, with
  `if: always()` (the pre-draft-gating form).
- Resolution (smallest correct change): keep upstream's newer draft-gated
  `if:` line and add the port's gate to `needs`:

```yaml
    needs: [changes, checks, clippy, build-tests, tests, doc-tests, oracle, windows-fxc, windows-startup, linux-aarch64-gates]
    if: always() && (github.event_name != 'pull_request' || !github.event.pull_request.draft)
```

- Verified after the rebase: `git diff upstream/main -- .github/workflows/ci.yml`
  shows only the intended additions — the `needs` entry, the `LINUX_AARCH64`
  environment entry and its `test ... = success` line, and the new
  `linux-aarch64-gates` job — and nothing else.

No other file conflicted: the four other replayed commits touch only new
`docs/ports/` files (plus, for `4e4c7b77`, those same new files), which
upstream does not have.

## Golden files

No conflict. `git diff upstream/main -- tests/robustness/goldens.json` is empty:
the branch carries upstream's records (`72b7fc38`, `f5a868ef`) unchanged. The
fork's old values and the upstream re-records are listed in
`upstream-golden-history.md`.

## Review note (not a rebase conflict; left unchanged)

The replayed `linux-aarch64-gates` job reads `needs.changes.outputs.native` in
its `if:` but, as authored on `port/arm64`, does not declare
`needs: [changes]` (upstream's `windows-fxc` and `windows-startup` both do; the
`native` output belongs to the `changes` job). A job's `needs` context only
contains the jobs listed in its `needs`, so the `if:` reads an empty context
and the job is skipped, while the `ci` gate's
`test "$LINUX_AARCH64" = success` would then fail on native changes. Left as
authored per this task's rules; flagged in `rebase-verification.md` under
"Findings".
