#!/usr/bin/env python3
"""
FreeCAD / ocp-freecad-cam toolpath adapter for solidcraft-cam.

Protocol
--------
Reads one JSON object from stdin, writes one JSON object to stdout.
No other output is written to stdout; diagnostic messages go to stderr.

Input schema:
  {
    "step_path":   "<absolute path to the STEP file written by solidcraft-cam>",
    "manufacture": { <ManufactureSettings JSON as defined in crates/manufacture> },
    "probe":       true | false   // optional; when true, only check imports & exit
  }

Output — success:
  {
    "status":      "ok",
    "segments":    [ { "kind": "rapid"|"linear"|"arc", ... } ],
    "post_output": "<G-code string>"   // optional, may be null
  }

Output — backend not installed:
  { "status": "unavailable", "message": "<reason>" }

Output — runtime error:
  { "status": "error", "message": "<reason>" }

Pinned versions (kept in sync with config.rs constants):
  FreeCAD:          1.1.4   (FREECAD_VERSION_PIN)
  ocp-freecad-cam:  0.1.0   (OCP_FREECAD_CAM_VERSION_PIN)

Licence: MIT OR Apache-2.0 (same as SolidCraft).
This script calls FreeCAD only through its public scripting API and does not
incorporate GPL code.  See tools/freecad-oracle/README.md for the project's
approach to FreeCAD as an external tool.

NOTE: Run this script through FreeCAD's Python interpreter so that the
`FreeCAD` and `Part` modules are importable.  Typical invocations:
  freecadcmd freecad_cam_adapter.py          (FreeCAD CLI interpreter)
  /opt/FreeCAD/bin/python freecad_cam_adapter.py
"""

from __future__ import annotations

import json
import os
import sys
import traceback
from typing import Any

FREECAD_PIN = "1.1.4"
OCP_FREECAD_CAM_PIN = "0.1.0"


# ── Output helpers ────────────────────────────────────────────────────────────

def _write(obj: dict[str, Any]) -> None:
    print(json.dumps(obj), flush=True)


def _unavailable(msg: str) -> None:
    _write({"status": "unavailable", "message": msg})


def _error(msg: str) -> None:
    _write({"status": "error", "message": msg})


def _ok(segments: list[dict[str, Any]], post_output: str | None = None) -> None:
    result: dict[str, Any] = {"status": "ok", "segments": segments}
    if post_output is not None:
        result["post_output"] = post_output
    _write(result)


# ── Entry point ───────────────────────────────────────────────────────────────

def main() -> None:
    try:
        payload = json.load(sys.stdin)
    except Exception as exc:
        _error(f"failed to parse stdin as JSON: {exc}")
        return

    probe = bool(payload.get("probe", False))

    # ── Import checks ─────────────────────────────────────────────────────────
    try:
        import FreeCAD  # noqa: F401
        import Part  # noqa: F401
    except ImportError as exc:
        _unavailable(
            f"FreeCAD not importable (is this running inside FreeCAD's Python? "
            f"Expected FreeCAD {FREECAD_PIN}): {exc}"
        )
        return

    try:
        import ocp_freecad_cam as cam  # type: ignore[import]
    except ImportError as exc:
        _unavailable(
            f"ocp-freecad-cam not importable "
            f"(install: pip install ocp-freecad-cam=={OCP_FREECAD_CAM_PIN}): {exc}"
        )
        return

    if probe:
        # Just confirm imports work; don't process any geometry.
        _ok([], None)
        return

    # ── Validate inputs ───────────────────────────────────────────────────────
    step_path: str = payload.get("step_path", "")
    if not step_path or not os.path.isabs(step_path) or not os.path.isfile(step_path):
        _error(f"step_path is invalid or not accessible: {step_path!r}")
        return

    manufacture: dict[str, Any] = payload.get("manufacture", {})
    setups: dict[str, Any] = manufacture.get("setups", {})
    if not setups:
        _ok([], None)
        return

    try:
        segments, post_output = process(step_path, setups, cam)
        _ok(segments, post_output)
    except Exception as exc:  # noqa: BLE001
        _error(f"toolpath computation failed: {exc}\n{traceback.format_exc()}")


# ── Core computation ──────────────────────────────────────────────────────────

def process(
    step_path: str,
    setups: dict[str, Any],
    cam: Any,
) -> tuple[list[dict[str, Any]], str | None]:
    """
    Load the STEP file and compute toolpath segments for every setup.

    Uses the ocp-freecad-cam 0.1.0 public API:
      https://github.com/voneiden/ocp-freecad-cam

    ocp-freecad-cam wraps FreeCAD's Path workbench.  The Job object is created
    from an OCC/CadQuery shape; each operation is chained on the Job.
    """
    import FreeCAD
    import Part
    import ocp_freecad_cam as cam  # re-import for type narrowing

    # Load the STEP file into a FreeCAD document.
    doc = FreeCAD.newDocument("cam_job")
    shape = Part.read(step_path)

    all_segments: list[dict[str, Any]] = []
    all_gcode_lines: list[str] = []

    for setup_name, setup in setups.items():
        try:
            segs, gcode = process_setup(doc, shape, setup_name, setup, cam)
            all_segments.extend(segs)
            if gcode:
                all_gcode_lines.append(f"; --- setup: {setup_name} ---")
                all_gcode_lines.append(gcode)
        except Exception as exc:  # noqa: BLE001
            # Surface per-setup errors as a warning in the output message.
            print(
                f"[solidcraft-cam adapter] setup {setup_name!r} failed: {exc}",
                file=sys.stderr,
            )
            raise

    FreeCAD.closeDocument(doc.Name)

    post_output = "\n".join(all_gcode_lines) if all_gcode_lines else None
    return all_segments, post_output


def process_setup(
    doc: Any,
    shape: Any,
    setup_name: str,
    setup: dict[str, Any],
    cam: Any,
) -> tuple[list[dict[str, Any]], str | None]:
    """
    Build a cam.Job for one setup, add all its operations, run the job.

    ocp-freecad-cam 0.1.0 API reference:
      cam.Job(shape, ...)       – create a job from an OCC/Part shape
      job.face(...)             – face-milling operation
      job.profile(...)          – profile/contour operation
      job.pocket(...)           – pocket clearing
      job.drill(...)            – drilling cycle
      job.adaptive(...)         – adaptive (trochoidal) clearing
      job.execute()             – compute toolpaths
      job.to_gcode()            – post-process to G-code string

    See the ocp-freecad-cam README for the full parameter reference.
    The parameters passed here mirror the solidcraft_manufacture domain types.
    """
    operations: list[dict[str, Any]] = setup.get("operations", [])
    tools: dict[int, dict[str, Any]] = {
        t["number"]: t for t in setup.get("tools", [])
    }

    # Build the Job from the shape.
    # ocp-freecad-cam expects a CadQuery or OCC workplane/shape.
    # FreeCAD's Part.Shape is directly accepted as of 0.1.0.
    try:
        job = cam.Job(shape)
    except Exception as exc:
        raise RuntimeError(f"cam.Job() failed: {exc}") from exc

    for op in operations:
        op_type: str = op.get("type", "")
        tool_number: int = op.get("tool_number", 1)
        tool_info: dict[str, Any] = tools.get(tool_number, {})
        feeds: dict[str, Any] = op.get("feeds", {})

        try:
            _add_operation(job, op_type, op, tool_info, feeds)
        except (AttributeError, TypeError, NotImplementedError) as exc:
            print(
                f"[solidcraft-cam adapter] operation {op_type!r} not supported "
                f"by ocp-freecad-cam {cam.__version__ if hasattr(cam, '__version__') else '?'}: {exc}",
                file=sys.stderr,
            )

    # Execute: compute toolpath geometry.
    try:
        job.execute()
    except AttributeError:
        # Some ocp-freecad-cam versions use .run() instead of .execute().
        try:
            job.run()
        except AttributeError as exc:
            raise RuntimeError(
                "cannot find job.execute() or job.run() — check ocp-freecad-cam version"
            ) from exc

    # Extract G-code via the post-processor.
    gcode: str | None = None
    try:
        gcode = job.to_gcode()
    except AttributeError:
        pass  # post_output is optional

    # Convert FreeCAD Path commands → PathSegment JSON.
    segments = extract_segments(job)
    return segments, gcode


def _add_operation(
    job: Any,
    op_type: str,
    op: dict[str, Any],
    tool: dict[str, Any],
    feeds: dict[str, Any],
) -> None:
    """Dispatch a single ManufactureSettings operation to the cam.Job API."""
    feedrate: float = feeds.get("feedrate", 1000.0)
    spindle_rpm: int = feeds.get("spindle_rpm", 10000)
    plunge: float = feeds.get("plunge", 300.0)

    if op_type == "face":
        depth: float = op.get("depth", 0.5)
        stepover: float = op.get("stepover", 0.75)
        # ocp-freecad-cam 0.1.0: job.face(depth_of_cut, stepover, ...)
        job.face(
            depth_of_cut=depth,
            stepover=stepover,
            feedrate=feedrate,
            spindle_rpm=spindle_rpm,
        )

    elif op_type == "profile":
        depth = op.get("depth", 3.0)
        side: str = op.get("side", "outside")
        # side → inside=True for pockets, outside=False
        inside = side == "inside"
        job.profile(
            depth_of_cut=depth,
            inside=inside,
            feedrate=feedrate,
            plunge_feedrate=plunge,
            spindle_rpm=spindle_rpm,
        )

    elif op_type == "pocket":
        depth = op.get("depth", 3.0)
        stepover = op.get("stepover", 0.45)
        job.pocket(
            depth_of_cut=depth,
            stepover=stepover,
            feedrate=feedrate,
            plunge_feedrate=plunge,
            spindle_rpm=spindle_rpm,
        )

    elif op_type == "drill":
        depth = op.get("depth", 10.0)
        job.drill(
            depth=depth,
            feedrate=plunge,  # drilling uses plunge rate
            spindle_rpm=spindle_rpm,
        )

    elif op_type == "adaptive":
        depth = op.get("depth", 4.0)
        optimal_load: float = op.get("optimal_load", 0.35)
        job.adaptive(
            depth_of_cut=depth,
            optimal_load=optimal_load,
            feedrate=feedrate,
            plunge_feedrate=plunge,
            spindle_rpm=spindle_rpm,
        )

    else:
        print(
            f"[solidcraft-cam adapter] unknown operation type {op_type!r}; skipping",
            file=sys.stderr,
        )


def extract_segments(job: Any) -> list[dict[str, Any]]:
    """
    Walk the FreeCAD Path commands on the job and convert each to a PathSegment.

    FreeCAD Path commands expose:
      cmd.Name       – "G0" (rapid), "G1" (linear), "G2"/"G3" (arc)
      cmd.Parameters – dict with X, Y, Z, F, I, J, K keys (float mm)

    PathSegment kinds (from solidcraft_manufacture):
      rapid  → { "kind": "rapid", "to": [x, y, z] }
      linear → { "kind": "linear", "to": [x, y, z], "feed": f }
      arc    → { "kind": "arc", "center": [cx,cy,cz], "to": [x,y,z],
                 "feed": f, "clockwise": bool }
    """
    segments: list[dict[str, Any]] = []

    # ocp-freecad-cam may expose the path via job.path or job.commands.
    path_obj = getattr(job, "path", None) or getattr(job, "commands", None)
    if path_obj is None:
        return segments

    commands = getattr(path_obj, "Commands", None) or path_obj
    if not commands:
        return segments

    for cmd in commands:
        name: str = getattr(cmd, "Name", "")
        params: dict[str, float] = getattr(cmd, "Parameters", {})

        to = [
            float(params.get("X", 0.0)),
            float(params.get("Y", 0.0)),
            float(params.get("Z", 0.0)),
        ]
        feed = float(params.get("F", 0.0))

        if name in ("G0", "G00"):
            segments.append({"kind": "rapid", "to": to})
        elif name in ("G1", "G01"):
            if feed <= 0.0:
                feed = 1000.0  # fallback
            segments.append({"kind": "linear", "to": to, "feed": feed})
        elif name in ("G2", "G02", "G3", "G03"):
            center = [
                float(params.get("I", 0.0)) + to[0],
                float(params.get("J", 0.0)) + to[1],
                float(params.get("K", 0.0)) + to[2],
            ]
            if feed <= 0.0:
                feed = 1000.0
            segments.append({
                "kind": "arc",
                "center": center,
                "to": to,
                "feed": feed,
                "clockwise": name in ("G2", "G02"),
            })
        # Other codes (G4 dwell, M-codes, etc.) are not PathSegments.

    return segments


if __name__ == "__main__":
    main()
