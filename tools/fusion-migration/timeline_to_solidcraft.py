#!/usr/bin/env python3
"""Translate a Fusion timeline (from ExtractTimeline.py) into a SolidCraft command script.

This is the part that makes a migrated part *editable* rather than a frozen solid. A STEP import
gives you geometry; this gives you the recipe -- parameters, sketches, and features that still
rebuild when you change a dimension.

    python timeline_to_solidcraft.py part.timeline.json -o part.json
    python timeline_to_solidcraft.py ~/cad/fusion-timeline --all      translate a whole tree
    python timeline_to_solidcraft.py part.timeline.json --verify      build it and report measures

Every step is either translated or recorded as untranslated with a reason. Nothing is dropped
quietly: a half-migrated part that claims to be whole is worse than one that admits the gap.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys

HOME = os.path.expanduser("~")


def find_cli():
    """Locate solidcraft-cli: $SOLIDCRAFT_CLI, PATH, a local cargo build, then a portable unzip."""
    env = os.environ.get("SOLIDCRAFT_CLI")
    if env and os.path.exists(env):
        return env
    exe = "solidcraft-cli.exe" if os.name == "nt" else "solidcraft-cli"
    found = shutil.which(exe) or shutil.which("solidcraft-cli")
    if found:
        return found
    here = os.path.dirname(os.path.abspath(__file__))
    for rel in ("../../target/release", "../../target/debug"):
        p = os.path.join(here, rel, exe)
        if os.path.exists(p):
            return os.path.abspath(p)
    import glob
    for p in sorted(glob.glob(os.path.join(
            os.path.expanduser("~"), "tools", "solidcraft", "*", exe)), reverse=True):
        return p
    return exe  # let the OS report it, with a name the user recognises


DEFAULT_CLI = find_cli()


# Fusion's FeatureOperations enum arrives as a number; these are its documented values.
OPERATION = {
    "0": "join", "1": "cut", "2": "intersect", "3": "new", "4": "new",
    "JoinFeatureOperation": "join", "CutFeatureOperation": "cut",
    "IntersectFeatureOperation": "intersect", "NewBodyFeatureOperation": "new",
    "NewComponentFeatureOperation": "new",
}

# FeatureExtentTypes.
EXTENT = {"0": "one_side", "1": "two_sides", "2": "symmetric"}

ORIGIN_PLANES = {"XY", "XZ", "YZ"}


class Skip(Exception):
    """This step cannot be rebuilt faithfully; say why rather than emit something wrong."""


def expr(v, default=None):
    """Prefer Fusion's expression over its number: the expression is what stays parametric."""
    if isinstance(v, dict):
        e = v.get("expression")
        if e:
            return e
        mm = v.get("value_mm")
        if mm is not None:
            return "{} mm".format(round(mm, 9))
    return default


def op_of(step, default="auto"):
    o = step.get("operation")
    return OPERATION.get(str(o), default) if o is not None else default


# ---------------------------------------------------------------------------
# sketches
# ---------------------------------------------------------------------------

def emit_sketch(step, out):
    sk = step.get("sketch") or {}
    plane = (sk.get("plane") or {}).get("name")
    if plane not in ORIGIN_PLANES:
        raise Skip(
            "sketch is on {}, not an origin plane -- face and offset-plane attachments need the "
            "parent geometry to exist first".format(plane or "an unnamed plane")
        )

    out.append({"command": "sketch.create", "params": {"plane": plane}})

    curves = sk.get("curves") or []
    if not curves:
        raise Skip("sketch has no readable curves")

    made = 0
    unsupported = {}
    for c in curves:
        kind = c.get("type")
        cons = bool(c.get("construction"))
        try:
            if kind == "line" and c.get("start") and c.get("end"):
                out.append({"command": "sketch.line",
                            "params": {"points": [c["start"], c["end"]], "construction": cons}})
            elif kind == "circle" and c.get("center") and c.get("radius_mm") is not None:
                out.append({"command": "sketch.circle.center",
                            "params": {"center": c["center"], "radius": c["radius_mm"],
                                       "construction": cons}})
            elif kind == "arc" and c.get("center") and c.get("start") and c.get("end"):
                out.append({"command": "sketch.arc.center_point",
                            "params": {"center": c["center"], "start": c["start"],
                                       "end": c["end"], "construction": cons}})
            elif kind in ("fitted_spline", "fixed_spline", "control_spline") and c.get("points"):
                pts = [p for p in c["points"] if p]
                if len(pts) < 2:
                    raise ValueError("too few points")
                out.append({"command": "sketch.spline.fit_point",
                            "params": {"points": pts, "construction": cons}})
            elif kind == "ellipse" and c.get("center"):
                raise ValueError("ellipse needs major/minor axes that were not captured")
            else:
                raise ValueError("no mapping")
            made += 1
        except Exception as e:
            unsupported[kind] = unsupported.get(kind, 0) + 1
            del e

    if not made:
        raise Skip("no curve in this sketch could be translated ({})".format(
            ", ".join("{}x{}".format(v, k) for k, v in unsupported.items()) or "empty"))

    return {"curves_made": made, "curves_skipped": unsupported} if unsupported else {"curves_made": made}


# ---------------------------------------------------------------------------
# features
# ---------------------------------------------------------------------------

def profiles_param(step):
    """Re-find each profile by a point inside it, which survives parameter changes."""
    profs = step.get("profiles")
    if not profs:
        return "all"
    pts = [{"point": p["point"]} for p in profs if p.get("point")]
    return pts or "all"


def emit_extrude(step, out):
    d = expr(step.get("distance"))
    if not d:
        raise Skip("no distance expression (extent may be to-object or through-all)")
    params = {"distance": d, "profiles": profiles_param(step), "operation": op_of(step)}
    t = expr(step.get("taper"))
    if t and not str(t).startswith("0"):
        params["taper"] = t
    if str(step.get("extent_type")) == "2" or step.get("is_symmetric") in (True, "True"):
        params["direction"] = "symmetric"
    d2 = expr(step.get("distance_two"))
    if d2:
        params["distance2"] = d2
    out.append({"command": "solid.extrude", "params": params})
    return {}


def emit_fillet(step, out):
    pts = step.get("edge_points")
    if not pts:
        raise Skip("no edge geometry captured ({} edges referenced)".format(
            step.get("edge_count", "?")))
    r = expr(step.get("radius"))
    if not r:
        raise Skip("no radius expression")
    out.append({"command": "solid.fillet", "params": {"edges": pts, "radius": r}})
    return {"edges": len(pts)}


def emit_chamfer(step, out):
    pts = step.get("edge_points")
    if not pts:
        raise Skip("no edge geometry captured")
    d = expr(step.get("distance")) or expr(step.get("radius"))
    if not d:
        raise Skip("no chamfer distance expression")
    out.append({"command": "solid.chamfer", "params": {"edges": pts, "distance": d}})
    return {"edges": len(pts)}


def emit_shell(step, out):
    pts = step.get("face_points") or []
    t = expr(step.get("thickness"))
    if not t:
        raise Skip("no thickness expression")
    out.append({"command": "solid.shell", "params": {"faces": pts, "thickness": t,
                                                     "direction": "inside"}})
    return {"faces": len(pts)}


def emit_draft(step, out):
    pts = step.get("face_points")
    if not pts:
        raise Skip("no face geometry captured")
    a = expr(step.get("angle"))
    if not a:
        raise Skip("no draft angle expression")
    out.append({"command": "solid.draft",
                "params": {"faces": pts, "angle": a, "neutral": "XY"}})
    return {"faces": len(pts)}


def emit_revolve(step, out):
    a = expr(step.get("angle"), "360 deg")
    out.append({"command": "solid.revolve",
                "params": {"axis": "x", "angle": a, "profiles": profiles_param(step),
                           "operation": op_of(step)}})
    return {"note": "axis guessed as sketch x -- check it"}


HANDLERS = {
    "Sketch": emit_sketch,
    "ExtrudeFeature": emit_extrude,
    "FilletFeature": emit_fillet,
    "ChamferFeature": emit_chamfer,
    "ShellFeature": emit_shell,
    "DraftFeature": emit_draft,
    "RevolveFeature": emit_revolve,
}


def translate(doc):
    """Timeline document -> (command script, coverage report)."""
    cmds = []
    steps = []

    for p in doc.get("parameters", []):
        prm = {"name": p["name"], "expression": p["expression"]}
        if p.get("unit"):
            prm["unit"] = p["unit"]
        if p.get("comment"):
            prm["comment"] = p["comment"]
        cmds.append({"command": "parameters.add", "params": prm})

    for step in doc.get("timeline", []):
        kind = step.get("entity") or "unknown"
        rec = {"index": step.get("index"), "name": step.get("name"), "entity": kind}

        if step.get("suppressed"):
            rec.update(status="skipped", reason="suppressed in Fusion")
            steps.append(rec)
            continue

        handler = HANDLERS.get(kind)
        if handler is None:
            rec.update(status="untranslated", reason="no mapping for {} yet".format(kind))
            steps.append(rec)
            continue

        mark = len(cmds)
        try:
            detail = handler(step, cmds) or {}
            rec.update(status="translated", commands=len(cmds) - mark, **detail)
        except Skip as e:
            del cmds[mark:]  # never leave half a feature behind
            rec.update(status="untranslated", reason=str(e))
        except Exception as e:
            del cmds[mark:]
            rec.update(status="error", reason="{}: {}".format(type(e).__name__, e))
        steps.append(rec)

    done = sum(1 for s in steps if s["status"] == "translated")
    report = {
        "design": doc.get("name"),
        "path": doc.get("path"),
        "units": doc.get("units"),
        "parameters": len(doc.get("parameters", [])),
        "steps_total": len(steps),
        "steps_translated": done,
        "coverage": round(done / len(steps), 4) if steps else 0.0,
        "commands": len(cmds),
        "steps": steps,
    }
    return {"commands": cmds}, report


def verify(cli, script_path):
    """Build the translated script and report what SolidCraft actually made of it."""
    # The CLI runs from its own directory, so the script must be addressed absolutely.
    script_path = os.path.abspath(script_path)
    try:
        p = subprocess.run([cli, "eval", script_path], capture_output=True, text=True,
                           timeout=300, cwd=os.path.dirname(cli) or None)
    except subprocess.TimeoutExpired:
        return {"ok": False, "error": "timeout"}
    txt = (p.stdout or "").strip()
    if not txt.startswith("{"):
        err = (p.stderr or p.stdout or "no output").strip().splitlines()
        return {"ok": False, "error": (err[0] if err else "no output")[:300]}
    d = json.loads(txt)
    t = d.get("total") or {}
    return {"ok": bool(d.get("body_count")), "bodies": d.get("body_count"),
            "volume_mm3": t.get("volume_mm3"), "area_mm2": t.get("area_mm2"),
            "faces": t.get("faces")}


def one(path, out_path, cli, do_verify):
    with open(path, encoding="utf-8") as fh:
        doc = json.load(fh)
    script, report = translate(doc)

    os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
    with open(out_path, "w", encoding="utf-8") as fh:
        json.dump(script, fh, indent=2)
    with open(os.path.splitext(out_path)[0] + ".coverage.json", "w", encoding="utf-8") as fh:
        json.dump(report, fh, indent=2)

    if do_verify:
        report["verify"] = verify(cli, out_path)
        with open(os.path.splitext(out_path)[0] + ".coverage.json", "w", encoding="utf-8") as fh:
            json.dump(report, fh, indent=2)
    return report


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("input", help=".timeline.json file, or a folder with --all")
    ap.add_argument("-o", "--out", default=None)
    ap.add_argument("--all", action="store_true", help="translate every timeline under a folder")
    ap.add_argument("--outdir", default=os.path.join(HOME, "cad", "rebuilt"))
    ap.add_argument("--verify", action="store_true", help="build each script and measure it")
    ap.add_argument("--cli", default=DEFAULT_CLI)
    args = ap.parse_args()

    if args.verify and not os.path.exists(args.cli):
        sys.exit("solidcraft-cli not found at {}".format(args.cli))

    if args.all:
        root = os.path.abspath(os.path.expanduser(args.input))
        files = []
        for dp, _d, names in os.walk(root):
            for n in names:
                if n.endswith(".timeline.json"):
                    files.append(os.path.join(dp, n))
        if not files:
            sys.exit(
                "no .timeline.json files under {}\n\n"
                "{}\n"
                "Run the ExtractTimeline script inside Fusion first:\n"
                "  Utilities > ADD-INS > Scripts and Add-Ins > ExtractTimeline > Run\n"
                "It writes one .timeline.json per design to ~/cad/fusion-timeline.".format(
                    root,
                    "That folder does not exist yet." if not os.path.isdir(root)
                    else "That folder exists but is empty.",
                )
            )
        files.sort()

        reports = []
        for f in files:
            rel = os.path.relpath(f, root).replace("\\", "/")
            stem = rel[: -len(".timeline.json")]
            out_path = os.path.join(args.outdir, stem + ".json")
            try:
                r = one(f, out_path, args.cli, args.verify)
            except Exception as e:
                print("FAIL {} - {}".format(rel, e))
                continue
            reports.append(r)
            v = r.get("verify") or {}
            vs = ""
            if args.verify:
                vs = " | built {} bodies".format(v.get("bodies")) if v.get("ok") else " | BUILD FAILED"
            print("{:>5.0%}  {:>3}/{:<3} steps  {}{}".format(
                r["coverage"], r["steps_translated"], r["steps_total"], stem[:48], vs))

        os.makedirs(args.outdir, exist_ok=True)
        with open(os.path.join(args.outdir, "rebuild-report.json"), "w", encoding="utf-8") as fh:
            json.dump({"designs": reports}, fh, indent=2)

        tot = sum(r["steps_total"] for r in reports)
        got = sum(r["steps_translated"] for r in reports)
        print("\n{} designs, {}/{} timeline steps translated ({:.0%})".format(
            len(reports), got, tot, (got / tot) if tot else 0))

        missing = {}
        for r in reports:
            for s in r["steps"]:
                if s["status"] != "translated":
                    missing[s["entity"]] = missing.get(s["entity"], 0) + 1
        if missing:
            print("\nmost common untranslated steps:")
            for k, v in sorted(missing.items(), key=lambda kv: -kv[1])[:12]:
                print("  {:<34} {}".format(k, v))
        print("\nreport: {}".format(os.path.join(args.outdir, "rebuild-report.json")))
        return 0

    out_path = args.out or os.path.splitext(os.path.splitext(args.input)[0])[0] + ".json"
    r = one(args.input, out_path, args.cli, args.verify)
    print("{}: {}/{} steps translated ({:.0%}), {} commands".format(
        r["design"], r["steps_translated"], r["steps_total"], r["coverage"], r["commands"]))
    for s in r["steps"]:
        if s["status"] != "translated":
            print("  {} #{} {} - {}".format(s["status"], s["index"], s["entity"],
                                            s.get("reason", "")[:90]))
    if args.verify:
        print("verify:", json.dumps(r.get("verify")))
    print("script: {}".format(out_path))
    return 0


if __name__ == "__main__":
    sys.exit(main())
