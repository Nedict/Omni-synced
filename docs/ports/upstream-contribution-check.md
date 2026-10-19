# Upstream contribution check

Read from `upstream/main:CONTRIBUTING.md` (fetched 2026-10-19; `main` at
`4c30d944`) before preparing the ARM64 port PR. Bullets list the rules that apply
to this branch and its eventual pull request; no rule found blocks the branch as
it stands.

- **Claim first.** Work is tracked in issues; claim with a draft PR
  (`claim: #N`), reference `Fixes #N`, and the lowest-numbered open PR for an
  issue wins. This branch has no upstream issue or PR yet: opening one starts
  with the claim.
- **Required checks.** `cargo xtask ci` (fmt, assets, licences, layers, Clippy
  `-D warnings`, tests, wasm build check) is what a PR must pass, and it should
  be run before pushing.
- **Heavy CI runs after review.** On push, a pull request gets the formatting
  and repository checks plus a quick `cargo check`; the full CI (build, tests,
  goldens, Clippy) runs only once a maintainer adds the `approved-for-ci`
  label, and every later push re-runs it. The run URLs in
  `rebase-verification.md` are fork-runner evidence, not a substitute for that
  label-gated upstream CI.
- **No DCO, no CLA, no sign-off.** The `## Licence` section says contributing
  implies the MIT OR Apache-2.0 licence; there is nothing to sign.
- **One topic per PR** ("keep the PR to one topic"; branch from the latest
  `main`).
- **CI changes from forks:** not addressed by CONTRIBUTING.md. Nothing forbids
  `.github/workflows` changes; the triage bot labels `.github/**` changes
  `area:infra`, and the normal review + `approved-for-ci` flow governs them
  (GitHub's standard fork-workflow approval setting may also apply).
- **ARM runners / new CI jobs:** no explicit approval rule found. The added
  `linux-aarch64-gates` job (ubuntu-22.04-arm) is subject only to the same
  review + label flow.
- **Stay current.** `git pull --rebase origin main` before pushing, no force
  pushes; keep checking upstream and report conflicts and validation. Upstream
  `main` has moved past this branch's base (`e4ff62c8` → `4c30d944` at fetch
  time), so the PR branch must be rebased before it is opened.
- **Test profile.** CI runs tests with `--profile ci` (truck's debug assertions
  off); a test that fails only in a plain debug build must be called out in the
  PR. Goldens are re-recorded in the same PR only when a geometry change moves
  them; this branch changes no code under `crates/` or `tests/` and no goldens.
- **Evidence.** Observable-behavior changes need screenshots or a recording
  (agents capture their own); changes with no useful observable behavior do not.
  This branch is docs + CI only; its evidence is the committed run logs.
