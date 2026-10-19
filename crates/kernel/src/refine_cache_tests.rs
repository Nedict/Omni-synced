use super::*;

fn bits(v: Vec3) -> [u64; 3] {
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

fn check_body(body: &Body) -> usize {
    let meshes = mesh_shells(&body.solid, 0.5);
    let mut changed = 0;
    for (face, meshed) in body.solid.face_iter().zip(meshes.iter().flat_map(|shell| shell.face_iter())) {
        let Some(mesh) = meshed.surface() else { continue };
        let surface = face.surface();
        let cached = refined_with_cache(&mesh, &surface, 0.002, true);
        let original = refined_with_cache(&mesh, &surface, 0.002, false);
        match (cached, original) {
            (None, None) => {}
            (Some(cached), Some(original)) => {
                changed += 1;
                assert_eq!(cached.len(), original.len(), "triangle count/winding must not change");
                for (cached, original) in cached.into_iter().zip(original) {
                    for ((point, normal), (old_point, old_normal)) in cached.into_iter().zip(original) {
                        assert_eq!(bits(point), bits(old_point), "every refined coordinate must remain bit-identical");
                        assert_eq!(bits(normal), bits(old_normal), "every refined normal must remain bit-identical");
                    }
                }
            }
            _ => panic!("memoization changed the refine/no-refine decision"),
        }
    }
    changed
}

#[test]
fn exact_refinement_cache_preserves_periodic_seams_and_normals() {
    let cylinder = crate::cylinder(Vec3::ZERO, Vec3::Z, 15., 8.).unwrap();
    let torus = crate::torus(Vec3::ZERO, 18., 4.).unwrap();
    assert!(check_body(&cylinder) > 0, "the fixture must actually refine");
    assert!(check_body(&torus) > 0, "the periodic fixture must actually refine");
}

#[test]
fn exact_refinement_cache_preserves_singular_poles_and_placed_surfaces() {
    let sphere = crate::sphere(Vec3::ZERO, 12.).unwrap();
    assert!(check_body(&sphere) > 0, "the polar fixture must actually refine");
    let placed = crate::transform(&sphere, Vec3::new(120., -85., 40.), Vec3::ZERO, Vec3::new(1., 2., 3.), 0.371).unwrap();
    assert!(check_body(&placed) > 0);
}

#[test]
fn exact_refinement_cache_keeps_unnecessary_planar_refinement_absent() {
    let body = crate::box_solid(Vec3::ZERO, Vec3::new(10., 20., 30.)).unwrap();
    assert_eq!(check_body(&body), 0);
}

/// Inspect the original U1 failure before a separately labelled second mesh probe.
#[test]
fn u1_face_diagnostics_after_first_validity() {
    use mt::{ParametricCurve, ParametricSurface};
    use serde_json::json;

    fn hex(value: f64) -> String {
        format!("{:016x}", value.to_bits())
    }
    fn point_bits(point: mt::Point3) -> [String; 3] {
        [hex(point.x), hex(point.y), hex(point.z)]
    }
    fn cache_state(body: &Body) -> serde_json::Value {
        let volume = match &body.volume {
            None => json!({"state": "absent"}),
            Some(cell) => match cell.try_lock() {
                Ok(slot) => json!({
                    "state": "readable",
                    "entry": (*slot).map(|(tol, value)| json!({"tolerance_bits": format!("{tol:016x}"), "value_bits": hex(value)})),
                }),
                Err(error) => json!({"state": "unavailable", "error": error.to_string()}),
            },
        };
        json!({
            "coarse_missing": body.coarse_missing.map(|(tol, count)| json!({"tolerance_bits": hex(tol), "missing_count": count})),
            "volume_cache": volume,
        })
    }

    // Same primitive construction and warm primitive measures as boolean_props U1.
    // No diagnostic tessellation or cache clearing precedes FIRST validity.
    let a = crate::sphere(Vec3::new(20.0, 10.0, 27.5), 3.75).unwrap();
    let b = crate::box_solid(Vec3::new(15.0, 0.0, 22.5), Vec3::new(22.5, 20.0, 27.5)).unwrap();
    let (va, vb) = (crate::measure(&a).unwrap().volume, crate::measure(&b).unwrap().volume);
    let (candidate, diagnostics) = crate::capture_boolean_diagnostics(|| {
        crate::boolean(&a, &b, crate::BoolOp::Union).map(|candidate| {
            candidate.map(|body| {
                let before = cache_state(&body);
                // validity itself calls measure internally, even when a face is missing.
                // Do not add the subsequent run() measure when validity rejects the body.
                let invalid = body.validity();
                let after = cache_state(&body);
                (body, before, invalid, after)
            })
        })
    });
    let (body, cache_before, invalid, cache_after) = match candidate {
        Ok(Some(observation)) => observation,
        other => {
            eprintln!(
                "U1_FACE_FIRST {}",
                json!({"order": "A_union_B", "phase": "boolean-error-or-empty", "result": format!("{other:?}"), "boolean_diagnostics": diagnostics})
            );
            panic!("U1 diagnostic needs the nonempty original Boolean result");
        }
    };
    let tol = orientation_tolerance(body.size());
    let unmeshed_tol = (body.size() * 1e-2).max(1e-3);
    let fine_tol = (tol * 0.02).max(1e-4);
    eprintln!(
        "U1_FACE_FIRST {}",
        json!({
            "order": "A_union_B", "phase": "first-validity-complete-before-second-probe",
            "primitive_volume_bits": [hex(va), hex(vb)], "invalid": invalid,
            "boolean_diagnostics": diagnostics, "cache_before_first_validity": cache_before,
            "cache_after_first_validity": cache_after, "size_bits": hex(body.size()),
        "orientation_tolerance_bits": hex(tol), "unmeshed_tolerance_bits": hex(unmeshed_tol),
        "cache_matches_unmeshed_tolerance": body.coarse_missing.is_some_and(|(cached_tol, _)| cached_tol == unmeshed_tol),
            "faces": body.face_count(), "edges": body.edge_count(), "shells": body.shell_count(),
            "original_missing_face_ids": null, "extra_measure_of_invalid_result": false,
        })
    );

    // This is a SECOND mesh, never the lost face index of a cached FIRST check.
    // Ordinals below belong to this exact body; truck IDs are process-local only.
    const MAX_FACES: usize = 32;
    const MAX_LOOPS: usize = 8;
    const MAX_EDGES_PER_LOOP: usize = 16;
    const MAX_EDGES_TOTAL: usize = 128;
    assert!(body.face_count() <= MAX_FACES && body.edge_count() <= MAX_EDGES_TOTAL, "U1 exceeds diagnostic topology budget");
    let second = guard("U1 SECOND diagnostic mesh and structure", || {
        let meshed = mesh_shells(&body.solid, tol);
        let report_mesh = |probe: &str, meshes: &[MeshedShell], tolerance: f64| {
            let missing: Vec<_> = meshes
                .iter()
                .flat_map(|shell| shell.face_iter())
                .enumerate()
                .filter(|(_, face)| face.surface().is_none_or(|mesh| mesh.faces().triangle_iter().next().is_none()))
                .map(|(index, _)| index)
                .collect();
            // Signed volume of this SECOND probe's raw mesh, as in Body::new's orient.
            // This is not measure(body), an accepted result volume or FIRST-pass evidence.
            let signed_volume = meshes.iter().map(|shell| shell.to_polygon().volume()).sum::<f64>();
            eprintln!(
                "U1_FACE_SECOND_MESH {}",
                json!({
                    "phase": "SECOND-probe-after-FIRST-validity", "probe": probe, "tolerance_bits": hex(tolerance),
                    "probe_mesh_signed_volume_bits": hex(signed_volume), "SECOND_missing_face_ordinals": missing,
                    "FIRST_missing_face_ids_known": false,
                })
            );
        };
        report_mesh("orientation-coarse", &meshed, tol);
        // Re-evaluating both tolerances cannot establish which constructor branch ran.
        // It can expose a cached count that disagrees with either fresh probe.
        let fine_meshed = mesh_shells(&body.solid, fine_tol);
        report_mesh("orientation-fine", &fine_meshed, fine_tol);
        drop(fine_meshed);
        let mesh_faces: Vec<_> = meshed.iter().flat_map(|shell| shell.face_iter()).collect();
        let face_count = body.face_count();
        if mesh_faces.len() != face_count {
            return Err(KernelError::Failed("U1 SECOND face enumeration differs from the source body".into()));
        }
        let mut rows = Vec::new();
        let mut reported_edges = 0;
        let mut missing = Vec::new();
        for (index, (face, meshed)) in body.solid.face_iter().zip(mesh_faces).enumerate() {
            let mesh = meshed.surface();
            let mesh_state = match &mesh {
                None => "none",
                Some(mesh) if mesh.faces().triangle_iter().next().is_none() => "no-triangles",
                Some(_) => "has-triangles",
            };
            if mesh_state != "has-triangles" {
                missing.push(index);
            }
            let surface = face.surface();
            let surface_kind = match &surface {
                mt::Surface::Plane(_) => "plane",
                mt::Surface::BSplineSurface(_) => "bspline",
                mt::Surface::NurbsSurface(_) => "nurbs",
                mt::Surface::RevolutedCurve(_) => "revoluted",
            };
            let range = |r: Option<(f64, f64)>| r.map(|(lo, hi)| [hex(lo), hex(hi)]);
            let (u, v) = surface.try_range_tuple();
            let mut loops = Vec::new();
            for (loop_index, wire) in face.absolute_boundaries().iter().take(MAX_LOOPS).enumerate() {
                let mut edges = Vec::new();
                let allowance = MAX_EDGES_PER_LOOP.min(MAX_EDGES_TOTAL - reported_edges);
                for (edge_index, edge) in wire.iter().take(allowance).enumerate() {
                    let curve = edge.curve();
                    let curve_kind = match &curve {
                        mt::Curve::Line(_) => "line",
                        mt::Curve::BSplineCurve(_) => "bspline",
                        mt::Curve::NurbsCurve(_) => "nurbs",
                        mt::Curve::IntersectionCurve(_) => "intersection",
                    };
                    edges.push(json!({
                        "edge_in_loop": edge_index, "process_local_edge_id": format!("{:?}", edge.id()),
                        "orientation": edge.orientation(), "curve_kind": curve_kind,
                        "range_bits": range(curve.try_range_tuple()),
                        "front_bits": point_bits(edge.front().point()), "back_bits": point_bits(edge.back().point()),
                    }));
                    reported_edges += 1;
                }
                loops.push(json!({"loop": loop_index, "edge_count": wire.len(), "truncated": edges.len() != wire.len(), "edges": edges}));
            }
            rows.push(json!({
                "body_face": index, "process_local_face_id": format!("{:?}", face.id()),
                "orientation": face.orientation(), "surface_kind": surface_kind,
                "surface_u_range_bits": range(u), "surface_v_range_bits": range(v),
                "surface_u_period_bits": surface.u_period().map(hex), "surface_v_period_bits": surface.v_period().map(hex),
                "loop_count": face.absolute_boundaries().len(), "loops_truncated": loops.len() != face.absolute_boundaries().len(),
                "loops": loops, "SECOND_mesh_state": mesh_state,
            }));
        }
        Ok(json!({
            "phase": "SECOND-probe-after-FIRST-validity", "order": "A_union_B", "tolerance_bits": hex(tol),
            "SECOND_missing_face_ordinals": missing, "FIRST_missing_face_ids_known": false,
            "max_faces": MAX_FACES, "max_loops_per_face": MAX_LOOPS, "max_edges_total": MAX_EDGES_TOTAL,
            "reported_edges": reported_edges, "faces": rows, "cache_after_SECOND": cache_state(&body),
        }))
    });
    match second {
        Ok(report) => {
            let report = serde_json::to_string(&report).unwrap();
            assert!(report.len() <= 256 * 1024, "U1 SECOND report exceeds byte budget");
            eprintln!("U1_FACE_SECOND {report}");
        }
        Err(error) => {
            eprintln!("U1_FACE_SECOND {}", json!({"phase": "SECOND-probe-error", "error": error.to_string()}));
            panic!("U1 SECOND diagnostic probe failed");
        }
    }
    // Reconstruct the default edge policy on this actual returned body.
    // This is post-FIRST/SECOND evidence, not a trace of private CDT internals.
    let causal = guard("U1 two-edge sampling witness and shared coarse mesh", || {
        use mt::BoundedCurve;
        use std::collections::{HashMap, HashSet};
        use truck_meshalgo::tessellation::{TrySubs, bounded_polyline, project_trim_point};

        let mut witnesses = Vec::new();
        let mut exercised_midpoint_case = false;
        for (face_index, face) in body.solid.face_iter().enumerate() {
            for wire in face.absolute_boundaries().iter().filter(|wire| wire.len() == 2) {
                if witnesses.len() >= 4 {
                    return Err(KernelError::Failed("U1 causal two-edge face budget exceeded".into()));
                }
                let surface = face.surface();
                let mut boundary = Vec::new();
                let mut edge_records = Vec::new();
                for edge in wire.edge_iter() {
                    let curve = edge.curve();
                    let range = curve.range_tuple();
                    let midpoint = curve.try_subs(0.5 * (range.0 + range.1));
                    // The public-hidden wrapper deliberately retains the old/default policy.
                    let mut points = bounded_polyline(&curve, range, Deflection { rulings: true, ..Deflection::chord(tol) }).0;
                    if !edge.orientation() {
                        points.reverse();
                    }
                    let midpoint_sag = midpoint.and_then(|mid| points.first().zip(points.last()).map(|(a, b)| mid.distance2(a.midpoint(*b)).sqrt()));
                    exercised_midpoint_case |= points.len() == 2
                        && midpoint.is_some_and(|p| p.x.is_finite() && p.y.is_finite() && p.z.is_finite())
                        && midpoint_sag.is_some_and(|sag| sag.is_finite() && sag > 0.0 && sag <= tol);
                    edge_records.push(json!({
                        "process_local_edge_id": format!("{:?}", edge.id()), "orientation": edge.orientation(),
                        "old_default_sample_count": points.len(), "range_bits": [hex(range.0), hex(range.1)],
                        "actual_curve_midpoint_bits": midpoint.map(point_bits),
                        "actual_curve_midpoint_sag_bits": midpoint_sag.map(hex),
                        "midpoint_finite": midpoint.is_some_and(|p| p.x.is_finite() && p.y.is_finite() && p.z.is_finite()),
                        "point_bits_first16": points.iter().take(16).copied().map(point_bits).collect::<Vec<_>>(),
                        "point_record_truncated": points.len() > 16,
                    }));
                    // Same endpoint omission as PolyBoundaryPiece. Do not project a prefix
                    // and present its area as a complete loop when the bound is exceeded.
                    if boundary.len() + points.len().saturating_sub(1) > 64 {
                        return Err(KernelError::Failed("U1 causal boundary-point budget exceeded".into()));
                    }
                    let take = points.len().saturating_sub(1);
                    boundary.extend(points.into_iter().take(take));
                }
                let mut hint: Option<(f64, f64)> = None;
                let mut uv = Vec::new();
                let mut complete = !boundary.is_empty();
                for point in boundary.iter().chain(boundary.first()) {
                    let Some((mut u, mut v)) = project_trim_point(&surface, *point, hint, tol) else {
                        complete = false;
                        break;
                    };
                    let unwrap = |x: f64, previous: f64, period: Option<f64>| match period.filter(|p| p.is_finite() && *p > 0.0) {
                        Some(period) => {
                            (-2..=2).map(|i| x + i as f64 * period).min_by(|a, b| (a - previous).abs().total_cmp(&(b - previous).abs())).unwrap_or(x)
                        }
                        None => x,
                    };
                    if let Some((pu, pv)) = hint {
                        u = unwrap(u, pu, surface.u_period());
                        v = unwrap(v, pv, surface.v_period());
                    }
                    if !(u.is_finite() && v.is_finite()) {
                        complete = false;
                        break;
                    }
                    uv.push((u, v));
                    hint = Some((u, v));
                }
                let area = complete.then(|| uv.windows(2).map(|w| w[0].0 * w[1].1 - w[1].0 * w[0].1).sum::<f64>() * 0.5);
                let distinct: HashSet<_> = uv.iter().map(|(u, v)| (u.to_bits(), v.to_bits())).collect();
                witnesses.push(json!({
                    "body_face": face_index, "phase": "old-default-policy-reconstruction-after-FIRST-SECOND",
                    "edges": edge_records, "projection_complete": complete,
                    "projected_uv_bits": uv.iter().map(|(u, v)| [hex(*u), hex(*v)]).collect::<Vec<_>>(),
                    "bit_distinct_uv_count": distinct.len(), "reconstructed_signed_uv_area_bits": area.map(hex),
                    "causal_proof": "not-established-by-reconstruction-alone",
                    "reconstruction_omits": ["singular derivative bridge vertices", "natural domain frame", "CDT filtering"],
                }));
            }
        }
        eprintln!(
            "U1_FACE_CAUSAL_SAMPLES {}",
            json!({
                "phase": "post-FIRST-SECOND-old-policy-reconstruction", "witnesses": &witnesses,
                "old_default_elided_finite_midpoint": exercised_midpoint_case,
            })
        );
        if witnesses.is_empty() || !exercised_midpoint_case {
            return Err(KernelError::Failed("U1 fixture did not exercise a two-edge wire with an elided finite curved midpoint".into()));
        }
        let meshes = mesh_shells(&body.solid, tol);
        let mut shared: HashMap<_, (Vec<[u64; 3]>, usize, i32)> = HashMap::new();
        let mut total_points = 0;
        let mut checked_faces = 0;
        let mut checked_incidences = 0;
        for face in meshes.iter().flat_map(|shell| shell.face_iter()) {
            let Some(mesh) = face.surface() else {
                return Err(KernelError::Failed("U1 repaired coarse face is unmeshed".into()));
            };
            if mesh.faces().triangle_iter().next().is_none()
                || mesh.positions().iter().any(|p| !(p.x.is_finite() && p.y.is_finite() && p.z.is_finite()))
                || mesh.normals().iter().any(|p| !(p.x.is_finite() && p.y.is_finite() && p.z.is_finite()))
                || mesh.uv_coords().iter().any(|p| !(p.x.is_finite() && p.y.is_finite()))
            {
                return Err(KernelError::Failed("U1 coarse face has no triangles or nonfinite attributes".into()));
            }
            checked_faces += 1;
            for edge in face.absolute_boundaries().iter().flat_map(|wire| wire.edge_iter()) {
                let points = edge.curve().0;
                if points.iter().any(|p| !(p.x.is_finite() && p.y.is_finite() && p.z.is_finite())) {
                    return Err(KernelError::Failed("U1 coarse shared edge has nonfinite samples".into()));
                }
                let point_bits = |p: &mt::Point3| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()];
                let direction = if edge.orientation() == face.orientation() { 1 } else { -1 };
                if let Some((previous, count, balance)) = shared.get_mut(&edge.id()) {
                    if !points.iter().map(point_bits).eq(previous.iter().copied()) {
                        return Err(KernelError::Failed("U1 incident faces disagree on shared edge samples".into()));
                    }
                    *count += 1;
                    *balance += direction;
                } else {
                    total_points += points.len();
                    if total_points > 262_144 {
                        return Err(KernelError::Failed("U1 shared-edge test budget exceeded".into()));
                    }
                    shared.insert(edge.id(), (points.iter().map(point_bits).collect::<Vec<_>>(), 1, direction));
                }
                checked_incidences += 1;
            }
        }
        if checked_faces != body.face_count() {
            return Err(KernelError::Failed("U1 coarse mesh omitted a source face".into()));
        }
        if shared.values().any(|(_, count, balance)| *count != 2 || *balance != 0) {
            return Err(KernelError::Failed("U1 coarse shared edges need two opposite face incidences".into()));
        }
        Ok(json!({
            "phase": "post-FIRST-SECOND-regression",
            "coarse_faces_with_finite_triangles": checked_faces, "shared_edges": shared.len(),
            "checked_edge_incidences": checked_incidences, "canonical_shared_point_count": total_points,
            "shared_comparison": "absolute edge curve points bit-identical; occurrence orientation is retained by each edge",
        }))
    });
    match causal {
        Ok(report) => {
            let report = serde_json::to_string(&report).unwrap();
            assert!(report.len() <= 128 * 1024, "U1 causal report exceeds byte budget");
            eprintln!("U1_FACE_CAUSAL {report}");
        }
        Err(error) => {
            eprintln!("U1_FACE_CAUSAL {}", json!({"phase": "causal-or-regression-failure", "error": error.to_string()}));
            panic!("U1 coarse shared-edge regression failed");
        }
    }
    if invalid.is_empty() {
        let physical = crate::measure(&body).unwrap();
        let radius: f64 = 3.75;
        let cap_height: f64 = 1.25;
        let sphere_volume = 4.0 * std::f64::consts::PI * radius.powi(3) / 3.0;
        let cap_volume = std::f64::consts::PI * cap_height.powi(2) * (radius - cap_height / 3.0);
        let expected = 750.0 + sphere_volume - (sphere_volume - cap_volume) * 0.5;
        eprintln!(
            "U1_FACE_VALID_MEASURE {}",
            json!({"phase": "measure-only-after-FIRST-validity-pass", "volume_bits": hex(physical.volume), "analytic_union_bits": hex(expected)})
        );
        // Same 2e-3 and primitive-sum scale as the unchanged boolean_props contract.
        assert!(
            physical.volume.is_finite() && (physical.volume - expected).abs() <= 2e-3 * (va + vb).max(1e-9),
            "U1 valid Union differs from the independent analytic volume"
        );
    } else {
        eprintln!("U1_FACE_VALID_MEASURE {}", json!({"phase": "skipped-FIRST-validity-failure", "volume_bits": null}));
    }

    // Preserve the original soundness requirement; emitting a diagnostic is not a pass.
    assert!(invalid.is_empty(), "U1 FIRST validity rejected the original result: {invalid:?}");
}
