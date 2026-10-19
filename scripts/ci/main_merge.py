#!/usr/bin/env python3
"""Is this pull request push only a merge of main into a change that already passed CI?

    python3 -I scripts/ci/main_merge.py --base BASE_SHA --head HEAD_SHA --repo OWNER/NAME

Prints `carried=<url of the passing run>` when all of these hold, else `carried=`:
  - HEAD is a merge commit whose second parent is on main (an ancestor of BASE): "Merge branch
    'main'", or GitHub's Update branch button;
  - the pull request's own change is the same before and after the merge (equal `git patch-id`),
    so the merge resolved no conflict and brought nothing into the change itself;
  - a ci.yml run of the first parent (the change before the merge) passed with the test shards.

Then the run needs only the repository checks: the change was tested, and main tests the merged
result after it lands. Any doubt (an error, an API failure, an unexpected shape) prints
`carried=` and the full checks run. Reads GH_TOKEN from the environment for the API.
"""
import argparse
import json
import os
import subprocess
import sys
import urllib.request

API = "https://api.github.com"
SHARD_PREFIX = "Native Linux tests"


def git(*args):
    return subprocess.run(["git", *args], capture_output=True, text=True, check=True).stdout


def patch_id(a, b):
    """The stable patch id of `git diff a b` ("" for an empty diff)."""
    diff = subprocess.run(["git", "diff", "--no-renames", a, b], capture_output=True, check=True).stdout
    if not diff.strip():
        return ""
    out = subprocess.run(["git", "patch-id", "--stable"], input=diff, capture_output=True, check=True).stdout
    return out.split()[0].decode() if out.split() else ""


def merge_of_main(base, head):
    """The first parent of HEAD when HEAD only merges main into an unchanged change, else None."""
    parents = git("rev-list", "--parents", "-n", "1", head).split()[1:]
    if len(parents) != 2:
        return None
    change, merged = parents
    if subprocess.run(["git", "merge-base", "--is-ancestor", merged, base]).returncode != 0:
        return None
    fork_point = git("merge-base", change, merged).strip()
    before, after = patch_id(fork_point, change), patch_id(merged, head)
    return change if before and before == after else None


def get(url, token):
    req = urllib.request.Request(url, headers={"Accept": "application/vnd.github+json", "Authorization": f"Bearer {token}"})
    with urllib.request.urlopen(req, timeout=20) as r:
        return json.load(r)


def passing_run(repo, sha, token):
    """URL of a successful pull request ci.yml run of `sha` whose test shards all passed."""
    runs = get(f"{API}/repos/{repo}/actions/workflows/ci.yml/runs?head_sha={sha}&event=pull_request&status=success&per_page=10", token)
    for run in runs.get("workflow_runs", []):
        jobs = get(f"{API}/repos/{repo}/actions/runs/{run['id']}/jobs?per_page=100", token).get("jobs", [])
        shards = [j for j in jobs if j.get("name", "").startswith(SHARD_PREFIX)]
        if shards and all(j.get("conclusion") == "success" for j in shards):
            return run.get("html_url", "")
    return ""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", required=True)
    ap.add_argument("--head", required=True)
    ap.add_argument("--repo", required=True)
    a = ap.parse_args()
    carried = ""
    try:
        change = merge_of_main(a.base, a.head)
        token = os.environ.get("GH_TOKEN", "")
        if change and token:
            carried = passing_run(a.repo, change, token)
            if carried:
                print(f"{a.head} merges main into {change}, which passed: {carried}", file=sys.stderr)
    except (subprocess.CalledProcessError, OSError, ValueError, KeyError) as e:
        print(f"not carried ({e})", file=sys.stderr)
        carried = ""
    print(f"carried={carried}")


if __name__ == "__main__":
    main()
