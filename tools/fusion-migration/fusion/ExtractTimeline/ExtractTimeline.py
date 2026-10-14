# ExtractTimeline - dump each design's parametric history to JSON, for rebuilding elsewhere.
#
# The geometry in a STEP export is a dead solid: no sketches, no parameters, nothing to edit.
# This asks Fusion for the recipe instead -- the ordered timeline, every sketch's curves,
# constraints and dimensions, and each feature's driving expressions -- so the design can be
# re-authored in another tool rather than merely re-imported.
#
# Writes <OUT>/<Hub>/<Project>/<folders>/<Design>.timeline.json, one file per design.
# Unknown feature types are still recorded, with whatever is readable, so nothing goes missing
# silently: a design is only as migratable as its least understood feature, and we want that
# visible rather than quietly dropped.
#
# Run from Fusion: Utilities > ADD-INS > Scripts and Add-Ins > ExtractTimeline > Run.

import adsk.core
import adsk.fusion
import traceback
import json
import os
import re
import time

# ---- settings -------------------------------------------------------------
OUT_DIR = os.path.expanduser("~/cad/fusion-timeline")
ALL_HUBS = False
SKETCH_GEOMETRY = True   # curve-by-curve sketch contents (the bulk of the output)

# Bumped whenever the captured shape changes, so re-running upgrades older files in place
# instead of skipping them -- a resumable run should not also mean a stale one.
SCHEMA = 3

# A plain text file of design paths (one per line, as written under OUT_DIR, without the
# ".timeline.json" suffix); only those are processed. Most hubs hold a lot of imported bodies
# with no history to recover, and re-reading those costs real time for nothing.
ONLY_LIST = os.path.join(OUT_DIR, "_reextract.txt")

# Try a few designs before committing to a whole hub. Each design has to be downloaded and
# loaded to be read, so a smoke test on five beats discovering a problem on five hundred.
LIMIT = 0        # 0 = every design; otherwise stop after this many
ONLY = ""        # case-insensitive substring of the design name or its folder path
# ---------------------------------------------------------------------------

_INVALID = re.compile(r'[<>:"/\\|?*\x00-\x1f]')
MM = 10.0  # Fusion works in centimetres; everything here is written in millimetres.


def safe(name):
    cleaned = _INVALID.sub("_", (name or "untitled")).strip().rstrip(".")
    if cleaned.upper().split(".")[0] in {
        "CON", "PRN", "AUX", "NUL",
        *(f"COM{i}" for i in range(1, 10)),
        *(f"LPT{i}" for i in range(1, 10)),
    }:
        cleaned = "_" + cleaned
    return cleaned[:120] or "untitled"


def pt(p):
    """A point in millimetres."""
    if p is None:
        return None
    try:
        return [round(p.x * MM, 9), round(p.y * MM, 9), round(p.z * MM, 9)]
    except Exception:
        return None


def pt2(p):
    if p is None:
        return None
    try:
        return [round(p.x * MM, 9), round(p.y * MM, 9)]
    except Exception:
        return None


def get(obj, *names):
    """First readable attribute of several, or None. Fusion raises on many properties."""
    for n in names:
        try:
            v = getattr(obj, n)
            if v is not None:
                return v
        except Exception:
            continue
    return None


def expr_of(value_input_owner, *names):
    """A feature's driving *expression* ('width/2 + 3 mm'), not just its number.

    The expression is the whole point: it is what makes the rebuilt design parametric.
    """
    for n in names:
        try:
            o = getattr(value_input_owner, n)
        except Exception:
            continue
        if o is None:
            continue
        try:
            return {"expression": o.expression, "value_mm": round(o.value * MM, 9)}
        except Exception:
            try:
                return {"expression": str(o.expression)}
            except Exception:
                continue
    return None


def count_of(coll):
    try:
        return coll.count
    except Exception:
        return 1


def as_items(coll):
    """Fusion hands back a single entity or a collection depending on the feature."""
    try:
        return [coll.item(i) for i in range(coll.count)]
    except Exception:
        return [coll]


def picks(coll, point_attr):
    """A world-space point lying on each edge or face, in millimetres.

    This is the whole trick for re-creating fillets, chamfers, shells and drafts somewhere
    else: the identity of an edge does not survive the move, but a point on it does.
    """
    out = []
    for item in as_items(coll):
        p = get(item, point_attr)
        if p is None:
            continue
        # Some builds expose it as a method rather than a property.
        if callable(p):
            try:
                p = p()
            except Exception:
                continue
        q = pt(p)
        if q:
            out.append(q)
    return out


def edge_sets(ent):
    """Edge picks for fillets and chamfers, which do not keep their edges where you'd expect.

    A FilletFeature holds `edgeSets`, and each set carries its own radius -- a single feature can
    round several groups of edges to different radii. Chamfers are the same shape on newer
    builds. Returns one record per set so each can be re-created with its own size, falling back
    to a plain `edges` collection where that is all there is.
    """
    sets = []
    for attr in ("edgeSets", "filletEdgeSets", "chamferEdgeSets"):
        coll = get(ent, attr)
        if coll is None:
            continue
        for s in as_items(coll):
            pts = picks(get(s, "edges") or [], "pointOnEdge")
            if not pts:
                continue
            rec = {"edge_points": pts, "kind": type(s).__name__}
            for key, names in (("radius", ("radius", "radiusOne")),
                               ("radius2", ("radiusTwo",)),
                               ("distance", ("distance", "distanceOne")),
                               ("distance2", ("distanceTwo",)),
                               ("angle", ("angle",))):
                v = expr_of(s, *names)
                if v:
                    rec[key] = v
            sets.append(rec)
        if sets:
            return sets

    # Older shape: the edges hang directly off the feature.
    for attr in ("edges", "inputEntities"):
        coll = get(ent, attr)
        if coll is None:
            continue
        pts = picks(coll, "pointOnEdge")
        if pts:
            return [{"edge_points": pts, "kind": "direct"}]
    return sets


def axis_of(ent):
    """A revolve's axis as two points in model space.

    The axis is usually a sketch line or a construction axis, neither of which keeps its identity
    across a migration. Its geometry does, and two points define the same axis anywhere.
    """
    for attr in ("axis", "axisEntity", "centerAxis"):
        a = get(ent, attr)
        if a is None:
            continue
        g = get(a, "geometry", "worldGeometry")
        if g is None:
            continue
        start, end = pt(get(g, "startPoint")), pt(get(g, "endPoint"))
        if start and end:
            return {"start": start, "end": end, "kind": type(a).__name__}
        origin, direction = get(g, "origin"), get(g, "direction")
        if origin is not None and direction is not None:
            o, d = pt(origin), pt(direction)
            if o and d:
                return {"start": o, "end": [o[i] + d[i] for i in range(3)],
                        "kind": type(a).__name__}
    return None


def path_of(ent):
    """A sweep's path as the points along its curves, so the route survives the move."""
    p = get(ent, "path")
    if p is None:
        return None
    pts = []
    try:
        for i in range(p.count):
            c = p.item(i)
            g = get(c, "geometry", "worldGeometry")
            if g is None:
                continue
            for attr in ("startPoint", "endPoint"):
                q = pt(get(g, attr))
                if q and (not pts or q != pts[-1]):
                    pts.append(q)
    except Exception:
        return None
    return pts or None


def profile_picks(prof):
    """Each profile as a point inside it, in its own sketch's 2D coordinates.

    Tools re-find a profile by a point inside it, which keeps following the right region when
    parameters change -- far more robust than a stored index.
    """
    out = []
    for p in as_items(prof):
        try:
            sk = p.parentSketch
            centroid = p.areaProperties().centroid
            local = sk.modelToSketchSpace(centroid)
            rec = {"sketch": get(sk, "name"), "point": pt2(local)}
            try:
                rec["area_mm2"] = round(p.areaProperties().area * MM * MM, 6)
            except Exception:
                pass
            if rec["point"]:
                out.append(rec)
        except Exception:
            continue
    return out


# ---------------------------------------------------------------------------
# sketches
# ---------------------------------------------------------------------------

def sketch_curves(sk):
    out = []
    try:
        curves = sk.sketchCurves
    except Exception:
        return out

    for coll, kind in (
        (get(curves, "sketchLines"), "line"),
        (get(curves, "sketchCircles"), "circle"),
        (get(curves, "sketchArcs"), "arc"),
        (get(curves, "sketchEllipses"), "ellipse"),
        (get(curves, "sketchFittedSplines"), "fitted_spline"),
        (get(curves, "sketchFixedSplines"), "fixed_spline"),
        (get(curves, "sketchControlPointSplines"), "control_spline"),
        (get(curves, "sketchConicCurves"), "conic"),
    ):
        if coll is None:
            continue
        for i in range(coll.count):
            c = coll.item(i)
            rec = {"type": kind, "construction": bool(get(c, "isConstruction"))}
            try:
                if kind == "line":
                    rec["start"] = pt2(c.startSketchPoint.geometry)
                    rec["end"] = pt2(c.endSketchPoint.geometry)
                elif kind == "circle":
                    rec["center"] = pt2(c.centerSketchPoint.geometry)
                    rec["radius_mm"] = round(c.radius * MM, 9)
                elif kind == "arc":
                    rec["center"] = pt2(c.centerSketchPoint.geometry)
                    rec["start"] = pt2(c.startSketchPoint.geometry)
                    rec["end"] = pt2(c.endSketchPoint.geometry)
                    rec["radius_mm"] = round(c.radius * MM, 9)
                elif kind == "ellipse":
                    rec["center"] = pt2(c.centerSketchPoint.geometry)
                else:
                    # Splines and conics: record the defining points so the shape can be
                    # rebuilt even where the tool has no identical curve type.
                    fp = get(c, "fitPoints", "controlPoints")
                    if fp is not None:
                        rec["points"] = [pt2(fp.item(k).geometry) for k in range(fp.count)]
            except Exception as e:
                rec["read_error"] = str(e)
            out.append(rec)
    return out


def sketch_constraints(sk):
    out = []
    try:
        cons = sk.geometricConstraints
    except Exception:
        return out
    for i in range(cons.count):
        c = cons.item(i)
        name = type(c).__name__
        name = re.sub(r"^.*?([A-Za-z]+)Constraint$", r"\1", name) or name
        out.append({"type": name.lower()})
    return out


def sketch_dimensions(sk):
    out = []
    try:
        dims = sk.sketchDimensions
    except Exception:
        return out
    for i in range(dims.count):
        d = dims.item(i)
        rec = {"type": re.sub(r"^Sketch|Dimension$", "", type(d).__name__).lower()}
        try:
            p = d.parameter
            rec["expression"] = p.expression
            rec["value_mm"] = round(p.value * MM, 9)
            rec["name"] = p.name
        except Exception:
            pass
        out.append(rec)
    return out


def describe_sketch(sk):
    rec = {
        "name": get(sk, "name"),
        "plane": None,
        "is_visible": bool(get(sk, "isVisible")),
    }
    try:
        ref = sk.referencePlane
        rec["plane"] = {"name": get(ref, "name"), "kind": type(ref).__name__}
    except Exception:
        pass
    try:
        t = sk.transform.asArray()
        rec["transform"] = [round(v, 9) for v in t]
    except Exception:
        pass
    if SKETCH_GEOMETRY:
        rec["curves"] = sketch_curves(sk)
        rec["constraints"] = sketch_constraints(sk)
        rec["dimensions"] = sketch_dimensions(sk)
        try:
            rec["profile_count"] = sk.profiles.count
        except Exception:
            pass
    return rec


# ---------------------------------------------------------------------------
# features
# ---------------------------------------------------------------------------

def describe_feature(ent):
    """Pull the parametric intent out of one timeline entity."""
    kind = type(ent).__name__
    rec = {"entity": kind, "name": get(ent, "name")}

    # The driving expressions, wherever this feature keeps them.
    for key, names in (
        ("distance", ("distanceOne", "distance")),
        ("distance_two", ("distanceTwo",)),
        ("taper", ("taperAngleOne", "taperAngle")),
        ("angle", ("angle",)),
        ("radius", ("radius",)),
        ("thickness", ("insideThickness", "thickness")),
    ):
        v = expr_of(ent, *names)
        if v:
            rec[key] = v

    # Extent / operation style, which decides how the feature is re-created.
    for key, names in (
        ("operation", ("operation",)),
        ("extent_type", ("extentType",)),
        ("is_symmetric", ("isSymmetric",)),
        ("direction", ("extentDirection",)),
    ):
        v = get(ent, *names)
        if v is not None and not hasattr(v, "objectType"):
            rec[key] = str(v)

    # Extrudes keep their extent in a definition object on newer builds.
    ext = get(ent, "extentOne", "extentDefinition")
    if ext is not None:
        d = expr_of(ext, "distance")
        if d:
            rec.setdefault("distance", d)
        rec["extent_definition"] = type(ext).__name__

    # What the feature was applied to. Fusion's internal edge and face ids mean nothing in
    # another tool, but a *point lying on* each edge or face is geometry, and geometry travels.
    # Most CAD tools (SolidCraft included) select exactly that way, so this is the bridge that
    # lets a fillet or a shell be re-created rather than merely reported as missing.
    sets = edge_sets(ent)
    if sets:
        rec["edge_sets"] = sets
        rec["edge_count"] = sum(len(s["edge_points"]) for s in sets)
        # Flattened too, so a consumer that only wants "every edge" need not walk the sets.
        rec["edge_points"] = [p for s in sets for p in s["edge_points"]]
    else:
        edges = get(ent, "edges")
        if edges is not None:
            rec["edge_count"] = count_of(edges)
            rec["edge_points_missing"] = True

    faces = get(ent, "faces", "inputFaces")
    if faces is not None:
        pts = picks(faces, "pointOnFace")
        rec["face_count"] = len(pts) if pts else count_of(faces)
        if pts:
            rec["face_points"] = pts

    prof = get(ent, "profile")
    if prof is not None:
        info = profile_picks(prof)
        if info:
            rec["profiles"] = info
            rec["profile_count"] = len(info)
        else:
            rec["profile_count"] = count_of(prof)

    try:
        rec["suppressed"] = bool(ent.isSuppressed)
    except Exception:
        pass

    # Feature-specific geometry that would otherwise be unrecoverable.
    if "Revolve" in kind:
        a = axis_of(ent)
        if a:
            rec["axis"] = a
    if "Sweep" in kind:
        p = path_of(ent)
        if p:
            rec["path_points"] = p
    if "Thread" in kind:
        info = {}
        for key, names in (("designation", ("threadDesignation",)),
                           ("type", ("threadType",)),
                           ("class", ("threadClass",)),
                           ("is_internal", ("isInternal",)),
                           ("full_length", ("isFullLength",))):
            v = get(ent, *names)
            if v is not None and not hasattr(v, "objectType"):
                info[key] = str(v)
        d = expr_of(ent, "threadLength")
        if d:
            info["length"] = d
        if info:
            rec["thread"] = info

    # Sketches carry their whole contents.
    if kind == "Sketch":
        rec["sketch"] = describe_sketch(ent)

    return rec


def describe_design(design, data_file, parts):
    doc = {
        "schema": SCHEMA,
        "name": get(data_file, "name"),
        "id": get(data_file, "id"),
        "version": get(data_file, "versionNumber"),
        "path": "/".join(parts),
        "units": None,
        "parameters": [],
        "timeline": [],
        "components": [],
        "stats": {},
    }
    try:
        doc["units"] = design.unitsManager.defaultLengthUnits
    except Exception:
        pass

    # Parameters first: the timeline's expressions refer to them by name.
    try:
        for i in range(design.userParameters.count):
            p = design.userParameters.item(i)
            doc["parameters"].append({
                "name": p.name,
                "expression": p.expression,
                "unit": get(p, "unit"),
                "value": round(p.value, 9),
                "comment": get(p, "comment") or "",
            })
    except Exception as e:
        doc["parameters_error"] = str(e)

    # User parameters routinely refer to *model* parameters -- Fusion's auto-named dimensions
    # like "d4" ("outerDiameter = d4 + 58.8 mm"). Those are driven by features rather than set
    # by hand, so they cannot be re-created as inputs elsewhere; capture their values so a
    # translator can at least resolve the reference instead of emitting an undefined name.
    try:
        user_names = {p["name"] for p in doc["parameters"]}
        model = []
        allp = design.allParameters
        for i in range(allp.count):
            p = allp.item(i)
            name = get(p, "name")
            if not name or name in user_names:
                continue
            model.append({
                "name": name,
                "expression": get(p, "expression"),
                "unit": get(p, "unit"),
                "value": round(p.value, 9),
            })
        doc["model_parameters"] = model
    except Exception as e:
        doc["model_parameters_error"] = str(e)

    try:
        for i in range(design.allComponents.count):
            c = design.allComponents.item(i)
            doc["components"].append({
                "name": get(c, "name"),
                "bodies": c.bRepBodies.count,
                "sketches": c.sketches.count,
                "features": c.features.count,
            })
    except Exception:
        pass

    unknown = {}
    try:
        tl = design.timeline
        for i in range(tl.count):
            tobj = tl.item(i)
            rec = {"index": i}
            try:
                rec["name"] = tobj.name
            except Exception:
                pass
            try:
                rec["rolled_back"] = not tobj.isSnapshot and False
            except Exception:
                pass
            ent = None
            try:
                ent = tobj.entity
            except Exception as e:
                rec["entity_error"] = str(e)
            if ent is not None:
                try:
                    rec.update(describe_feature(ent))
                except Exception as e:
                    rec["describe_error"] = "{}: {}".format(type(e).__name__, e)
                    rec["entity"] = type(ent).__name__
            k = rec.get("entity", "unknown")
            unknown[k] = unknown.get(k, 0) + 1
            doc["timeline"].append(rec)
    except Exception as e:
        doc["timeline_error"] = str(e)

    doc["stats"] = {
        "timeline_steps": len(doc["timeline"]),
        "parameters": len(doc["parameters"]),
        "feature_types": unknown,
    }
    return doc


# ---------------------------------------------------------------------------

def collect(folder, rel_parts, found, stats=None):
    """Walk a folder for designs, recording what was seen so a miss can be explained."""
    if stats is None:
        stats = {}
    try:
        n = folder.dataFiles.count
    except Exception as e:
        stats.setdefault("folder_errors", []).append("{}: {}".format("/".join(rel_parts), e))
        return
    stats["folders"] = stats.get("folders", 0) + 1
    for i in range(n):
        try:
            f = folder.dataFiles.item(i)
        except Exception:
            continue
        stats["files"] = stats.get("files", 0) + 1
        try:
            ext = f.fileExtension
        except Exception as e:
            stats.setdefault("ext_errors", []).append(str(e))
            continue
        exts = stats.setdefault("extensions", {})
        exts[ext or "(none)"] = exts.get(ext or "(none)", 0) + 1
        if ext == "f3d":
            found.append((f, list(rel_parts)))
    try:
        for i in range(folder.dataFolders.count):
            sub = folder.dataFolders.item(i)
            collect(sub, rel_parts + [safe(sub.name)], found, stats)
    except Exception as e:
        stats.setdefault("folder_errors", []).append("{}: {}".format("/".join(rel_parts), e))


def run(context):
    ui = None
    try:
        app = adsk.core.Application.get()
        ui = app.userInterface

        signed_in = getattr(app.data, "isSignedIn", None)
        if signed_in is False:
            ui.messageBox("Not signed in to Autodesk. Sign in, then run this again.")
            return

        try:
            hubs = []
            if ALL_HUBS:
                for i in range(app.data.dataHubs.count):
                    hubs.append(app.data.dataHubs.item(i))
            else:
                hubs.append(app.data.activeHub)
            hubs = [h for h in hubs if h is not None]
        except Exception as e:
            ui.messageBox("Could not reach your Autodesk hub ({}).\nSign in and try again.".format(e))
            return
        if not hubs:
            ui.messageBox("No Autodesk hub available. Sign in and try again.")
            return

        designs = []
        stats = {"hubs": [], "projects": 0, "project_errors": []}
        for hub in hubs:
            if len(hubs) > 1:
                try:
                    app.data.activeHub = hub
                except Exception:
                    pass
            adsk.doEvents()
            stats["hubs"].append(get(hub, "name") or "(unnamed)")
            try:
                projects = app.data.dataProjects
                pcount = projects.count
            except Exception as e:
                stats["project_errors"].append("listing projects: {}".format(e))
                continue
            stats["projects"] += pcount
            for p in range(pcount):
                try:
                    proj = projects.item(p)
                    root = proj.rootFolder
                except Exception as e:
                    stats["project_errors"].append("project {}: {}".format(p, e))
                    continue
                collect(root, [safe(hub.name), safe(get(proj, "name") or "project")],
                        designs, stats)

        if not designs:
            # Say what was actually seen. "No designs found" with no evidence is the kind of
            # answer that sends you looking in the wrong place.
            exts = stats.get("extensions") or {}
            lines = [
                "No Fusion designs (.f3d) found.",
                "",
                "hub(s):   {}".format(", ".join(stats["hubs"]) or "none"),
                "projects: {}".format(stats["projects"]),
                "folders:  {}".format(stats.get("folders", 0)),
                "files:    {}".format(stats.get("files", 0)),
                "file types seen: {}".format(
                    ", ".join("{} x{}".format(k, v) for k, v in sorted(exts.items()))
                    or "none"),
            ]
            for key, label in (("project_errors", "project errors"),
                               ("folder_errors", "folder errors"),
                               ("ext_errors", "file errors")):
                errs = stats.get(key) or []
                if errs:
                    lines += ["", "{}:".format(label)] + ["  " + e for e in errs[:5]]
                    if len(errs) > 5:
                        lines.append("  ... and {} more".format(len(errs) - 5))
            if stats["projects"] == 0:
                lines += ["", "No projects at all usually means the hub list has not loaded.",
                          "Open the Data Panel in Fusion once, then run this again."]
            ui.messageBox("\n".join(lines))
            return

        found_all = len(designs)
        if ONLY:
            needle = ONLY.lower()
            designs = [(f, p) for (f, p) in designs
                       if needle in (get(f, "name") or "").lower()
                       or needle in "/".join(p).lower()]
            if not designs:
                ui.messageBox('Nothing matched ONLY = "{}" among {} designs.'.format(
                    ONLY, found_all))
                return
        if ONLY_LIST and os.path.exists(ONLY_LIST):
            try:
                with open(ONLY_LIST, encoding="utf-8") as fh:
                    wanted = {ln.strip().replace("\\", "/").lower()
                              for ln in fh if ln.strip()}
            except Exception:
                wanted = set()
            if wanted:
                designs = [(f, p) for (f, p) in designs
                           if "/".join(p + [safe(get(f, "name") or "")]).lower() in wanted]

        if LIMIT and LIMIT > 0:
            designs = designs[:LIMIT]

        # Scope is reported in the progress dialog's title, never in a message box. This script
        # exists to be started and left alone, and a modal here blocks the whole run until
        # someone comes back and clicks it -- which is the one thing an unattended batch job
        # must never do. Modal dialogs below are only ever at the very end.
        scope = "{} of {} designs".format(len(designs), found_all) \
            if len(designs) != found_all else "{} designs".format(found_all)

        os.makedirs(OUT_DIR, exist_ok=True)
        total = len(designs)
        progress = ui.createProgressDialog()
        progress.isCancelButtonShown = True
        progress.show("Extracting timelines - " + scope, "%v / %m   %p%", 0, total, 0)

        started = time.time()
        ok = skipped = failed = 0
        summary = []

        for idx, (data_file, parts) in enumerate(designs):
            if progress.wasCancelled:
                break
            progress.progressValue = idx
            progress.message = "%v / %m   %p%\n" + data_file.name[:60]
            adsk.doEvents()

            out_dir = os.path.join(OUT_DIR, *parts)
            os.makedirs(out_dir, exist_ok=True)
            dest = os.path.join(out_dir, safe(data_file.name) + ".timeline.json")
            if os.path.exists(dest):
                try:
                    with open(dest, encoding="utf-8") as fh:
                        if json.load(fh).get("schema", 0) >= SCHEMA:
                            skipped += 1
                            continue
                except Exception:
                    pass  # unreadable or older: re-extract it

            doc = None
            design = None
            try:
                for visible in (False, True):
                    try:
                        doc = app.documents.open(data_file, visible)
                    except Exception:
                        doc = None
                    if doc is None:
                        continue
                    adsk.doEvents()
                    design = adsk.fusion.Design.cast(
                        doc.products.itemByProductType("DesignProductType"))
                    if design is not None:
                        break
                    try:
                        doc.close(False)
                    except Exception:
                        pass
                    doc = None

                if design is None:
                    raise RuntimeError("no Design product")

                info = describe_design(design, data_file, parts)
                with open(dest, "w", encoding="utf-8") as fh:
                    json.dump(info, fh, indent=2)
                summary.append({
                    "design": info["name"],
                    "path": info["path"],
                    "steps": info["stats"]["timeline_steps"],
                    "parameters": info["stats"]["parameters"],
                    "feature_types": info["stats"]["feature_types"],
                })
                ok += 1
            except Exception as e:
                failed += 1
                summary.append({
                    "design": get(data_file, "name"),
                    "path": "/".join(parts),
                    "error": "{}: {}".format(type(e).__name__, e),
                })
            finally:
                if doc is not None:
                    try:
                        doc.close(False)
                    except Exception:
                        pass
                adsk.doEvents()

            with open(os.path.join(OUT_DIR, "summary.json"), "w", encoding="utf-8") as fh:
                json.dump({"extracted_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
                           "total_found": total, "designs": summary}, fh, indent=2)

        progress.hide()
        elapsed = int(time.time() - started)
        steps = sum(s.get("steps", 0) for s in summary)
        ui.messageBox(
            "Timeline extraction finished.\n\n"
            "designs: {}\nskipped (already done): {}\nfailed: {}\n"
            "timeline steps captured: {}\nelapsed: {}m {}s\n\n{}".format(
                ok, skipped, failed, steps, elapsed // 60, elapsed % 60, OUT_DIR))

    except Exception:
        if ui:
            ui.messageBox("Script failed:\n{}".format(traceback.format_exc()))
