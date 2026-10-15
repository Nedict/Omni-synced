#!/usr/bin/env python3
"""Assemble the migrated library: every design as a .solidcraft file, best route available.

Three tools come before this one -- ExtractTimeline and ExportAllDesigns inside Fusion, and
timeline_to_solidcraft.py here -- and each answers part of the question. This one answers the
whole of it: for each design, produce the best SolidCraft file obtainable, and say which route it
took and why.

Routes, in order of preference:

  parametric  the timeline rebuilt and built cleanly -- sketches, parameters and features intact,
              so changing a dimension still rebuilds the part
  geometry    no usable timeline, or the rebuild failed; the STEP export is imported instead,
              which is exact but frozen
  none        neither was available

A design with no timeline in Fusion is not a failure of this tool: 106 of 270 designs here were
imported bodies or assemblies that never had modelling history, and for those the STEP route is
the correct answer rather than a consolation.

    python build_library.py                 build into ~/cad/solidcraft-library
    python build_library.py --only Bracket  just the ones matching
"""

import argparse
import concurrent.futures
import json
import os
import shutil
import subprocess
import sys

HOME = os.path.expanduser("~")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
try:
    from timeline_to_solidcraft import find_cli
except ImportError:
    def find_cli():
        return os.environ.get("SOLIDCRAFT_CLI", "solidcraft-cli")


def run_cli(cli, args, timeout=600):
    try:
        p = subprocess.run([cli] + args, capture_output=True, text=True, timeout=timeout,
                           cwd=os.path.dirname(cli) or None)
        return p.returncode, p.stdout or "", p.stderr or ""
    except subprocess.TimeoutExpired:
        return 1, "", "timeout"


def measure(cli, path):
    code, out, err = run_cli(cli, ["eval", os.path.abspath(path)])
    txt = out.strip()
    if not txt.startswith("{"):
        return None, (err or out or "no output").strip().splitlines()[0][:200]
    try:
        d = json.loads(txt)
    except json.JSONDecodeError as e:
        return None, str(e)[:120]
    return d, None


def build_one(cli, rel, script, step, out_root):
    """Produce the best .solidcraft for one design."""
    rec = {"design": rel, "route": "none", "notes": []}
    dest = os.path.join(out_root, rel + ".solidcraft")
    os.makedirs(os.path.dirname(dest) or ".", exist_ok=True)

    # Preferred: the rebuilt timeline, which stays editable.
    if script and os.path.exists(script):
        code, out, err = run_cli(
            cli, ["run", os.path.abspath(script), "--quiet", "--save", os.path.abspath(dest)])
        if os.path.exists(dest):
            d, merr = measure(cli, dest)
            if d is not None:
                # Zero bodies is a real design, not a failure: sketch-only reference files are
                # common and migrate perfectly well.
                rec.update(route="parametric", bodies=d.get("body_count") or 0,
                           volume_mm3=(d.get("total") or {}).get("volume_mm3"),
                           steps=len(d.get("timeline") or []))
                if not d.get("body_count"):
                    rec["notes"].append("sketches only, no solid bodies")
                return rec, dest
            os.remove(dest)
            rec["notes"].append("rebuilt file unreadable: %s" % merr)
        else:
            first = (err or out).strip().splitlines()
            rec["notes"].append("timeline build failed: %s" % (first[-1][:150] if first else "?"))

    # Fallback: the exported geometry. Exact, but frozen.
    if step and os.path.exists(step):
        code, out, err = run_cli(
            cli, ["run", "--in", os.path.abspath(step), "--quiet", "--save",
                  os.path.abspath(dest)])
        if os.path.exists(dest):
            d, merr = measure(cli, dest)
            if d and d.get("body_count"):
                rec.update(route="geometry", bodies=d["body_count"],
                           volume_mm3=(d.get("total") or {}).get("volume_mm3"))
                return rec, dest
            os.remove(dest)
        rec["notes"].append("STEP import failed")
    elif not script:
        rec["notes"].append("no timeline and no STEP export")
    else:
        rec["notes"].append("no STEP export to fall back on -- run ExportAllDesigns in Fusion")

    return rec, None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--timelines", default=os.path.join(HOME, "cad", "fusion-timeline"))
    ap.add_argument("--scripts", default=os.path.join(HOME, "cad", "rebuilt"))
    ap.add_argument("--export", default=os.path.join(HOME, "cad", "fusion-export"))
    ap.add_argument("--out", default=os.path.join(HOME, "cad", "solidcraft-library"))
    ap.add_argument("--only", default=None)
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--cli", default=None)
    args = ap.parse_args()

    cli = args.cli or find_cli()
    if not os.path.exists(cli):
        sys.exit("solidcraft-cli not found at %s" % cli)

    # Every design we know about, from either source.
    designs = {}
    for root, _d, files in os.walk(args.timelines):
        for f in files:
            if f.endswith(".timeline.json"):
                rel = os.path.relpath(os.path.join(root, f), args.timelines)
                rel = rel.replace("\\", "/")[: -len(".timeline.json")]
                designs.setdefault(rel, {})["timeline"] = True
    if os.path.isdir(args.export):
        for root, _d, files in os.walk(args.export):
            for f in files:
                if f.lower().endswith((".step", ".stp")):
                    rel = os.path.relpath(os.path.join(root, f), args.export)
                    rel = os.path.splitext(rel.replace("\\", "/"))[0]
                    designs.setdefault(rel, {})["step"] = os.path.join(root, f)

    if args.only:
        designs = {k: v for k, v in designs.items() if args.only.lower() in k.lower()}
    if not designs:
        sys.exit("nothing to build -- run ExtractTimeline and/or ExportAllDesigns in Fusion first")

    print("building %d designs into %s\n" % (len(designs), args.out))
    os.makedirs(args.out, exist_ok=True)

    results = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        futs = {}
        for rel, src in sorted(designs.items()):
            script = os.path.join(args.scripts, rel + ".json")
            futs[pool.submit(build_one, cli, rel, script, src.get("step"), args.out)] = rel
        for fut in concurrent.futures.as_completed(futs):
            rec, _dest = fut.result()
            results.append(rec)
            mark = {"parametric": "timeline", "geometry": "geometry", "none": "  --    "}[rec["route"]]
            print("  %-10s %-54s %s" % (mark, rec["design"][:54],
                                        "; ".join(rec["notes"])[:50] if rec["route"] == "none" else ""))

    results.sort(key=lambda r: r["design"])
    with open(os.path.join(args.out, "library.json"), "w", encoding="utf-8") as fh:
        json.dump({"count": len(results), "designs": results}, fh, indent=2)

    n = {"parametric": 0, "geometry": 0, "none": 0}
    for r in results:
        n[r["route"]] += 1
    print("\n%d designs" % len(results))
    print("  %3d with timeline intact (editable)" % n["parametric"])
    print("  %3d geometry only (exact, frozen)" % n["geometry"])
    print("  %3d not migrated" % n["none"])
    if not os.path.isdir(args.export):
        print("\nNo STEP exports found. Running ExportAllDesigns in Fusion would give the"
              "\ngeometry route to every design that has no usable timeline.")
    print("\nlibrary: %s" % args.out)
    return 1 if n["none"] else 0


if __name__ == "__main__":
    sys.exit(main())
