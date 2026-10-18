# Reviewing pull requests

How a pull request gets from "opened" to "merged": the code is read first, every comment is
answered, and CI follows. The steps apply to every reviewer, person or agent.

## 1. Read the code

A review starts with the diff and the code around it, never with CI. Green CI says the tests
pass; it does not say the tests cover the change, or that the change is right.

Read `gh pr diff <N>` and open the files it touches in a checkout of `main`. For each change, ask:

- **Is it correct?** Name the input that breaks it: an edge case, an empty list, a touching
  loop, a reversed face. A finding is a concrete scenario ("a hole whose vertex touches the outer
  loop names the side from the wrong curve"), not a feeling.
- **Does it keep the rules?** [CLAUDE.md](../CLAUDE.md) and [CONTRIBUTING.md](../CONTRIBUTING.md):
  no panics outside tests; input-derived numbers checked; `truck` only through the kernel guard;
  layering; a design-format change is one new file in `crates/doc/src/format/pending/`; a
  vendored edit is marked and listed in `vendor/README.md`; assets have an ATTRIBUTION.md row.
- **Do the tests prove it?** A bug fix lands with a test that fails without it. Expected values
  are analytic or from an open-source tool. A loosened tolerance or a re-recorded robustness
  golden needs its reason in the PR: what moved, and why that is right. A golden that *gains*
  volume from faceting, or builds a different list of steps, is a finding, not a blessing.
- **Is it one topic?** Unrelated files, merge commits from an old base or leftover claim
  commits are asked to go.

Delegating the reading to a helper (another agent, a tool) is fine, but its findings are leads.
Before a finding goes into a comment, the reviewer checks it in the code or reproduces it.

## 2. Answer what is already there

Read the whole conversation first: `gh pr view <N> --comments` and the inline comments
(`gh api repos/<owner>/<repo>/pulls/<N>/comments`). For each earlier ask, say whether the latest
commits resolve it, with the commit or line that does. An ask that commits since the review do
not touch (merges of `main` only) is still open.

## 3. Write the comment

Comments go to the people who built the change. Write them that way:

- **Thank them first, and name the specific good thing** (one or two lines): "pulling the
  coalescing rules into one `Coalesce::of` is much easier to follow".
- **Then the asks, as a short list.** For each: what is wrong, the scenario that shows it, and
  where and how to fix it (file, function, test to add, command to reproduce).
- **Mark what blocks the merge.** Don't soften a blocker into a nit, and don't present a nit as
  a blocker.
- **Only verified findings.** Say how it was verified ("fails on the stack, passes on the commit
  before"); a point that is only a suspicion is phrased as a question.
- **Reproduce commands must work as written.** Check them; never put a command in a comment
  that changes state (for example `ROBUSTNESS_GOLDENS_BLESS=` with no reason).
- No footers, no boilerplate. If a comment turns out wrong, correct it on the PR.

Label the PR `ready to merge` or `needs changes` to match.

## 4. CI follows

Run CI only on changes whose code review passed.

- **Stack to save runners.** PRs that touch different files can be applied together on the
  latest `main` (one commit per PR) and tested in one run instead of one run each. Put a PR
  that fixes the baseline first. Resolve `mod`-line and list conflicts by keeping both entries
  in order.
- **Compare with the baseline.** Know what fails on `main` alone on that machine before
  blaming a PR. Platform noise (some robustness seeds on macOS), wall-clock limits on a loaded
  machine and tests that need a sandbox-free environment are re-checked, not attributed.
- **Attribute new failures commit by commit.** When the stack adds a failure, run the failing
  tests on each stacked commit; the first commit that fails owns it.
- **GitHub CI on Linux decides.** The local stack finds problems early; the PR's own checks
  (`gh pr checks <N> --watch --fail-fast`) are the gate. A job killed by the runner (rustc exits
  with no diagnostic, a cancelled check) is re-run, and it is not counted as a pass.
- On a shared machine, build into the main checkout's `target/`; don't give each worktree its
  own.

## 5. Merge

A PR is merged when all three hold: the code review approved it, the local stack added no
failure of its own, and its GitHub checks are green. Merge one at a time with
`gh pr merge <N> --squash`, re-checking the next PR is still mergeable after each.
