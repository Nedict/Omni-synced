//! Opt-in acceptance gate for real user archives. A decoded index is not an import.
use serde_json::json;
use solidcraft_engine::{Session, io};
use std::path::{Path, PathBuf};

fn files(root: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    assert!(depth <= 8 && out.len() <= 128, "scope this gate to the F3D test corpus");
    for entry in std::fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let kind = entry.file_type().unwrap();
        let path = entry.path();
        if kind.is_dir() {
            files(&path, depth + 1, out);
        } else if kind.is_file() && io::f3d::is_f3d_path(&path.to_string_lossy()) {
            out.push(path);
        }
    }
}

fn check(path: &Path) -> Result<(usize, f64), String> {
    let mut session = Session::default();
    session.execute("doc.open", &json!({"path":path})).map_err(|e| e.to_string())?;
    if session.doc.features.is_empty() || session.model.results.iter().any(|r| r.error.is_some()) {
        return Err("import has no editable history or has regeneration errors".into());
    }
    let measured = session.execute("inspect.measure", &json!({})).map_err(|e| e.to_string())?;
    let volume = measured["total"]["volume_mm3"].as_f64().filter(|v| v.is_finite() && *v > 0.).ok_or("missing positive solid volume")?;
    let bytes = io::write_design_container(&session.doc).map_err(|e| e.to_string())?;
    let document = io::read_design(&bytes).map_err(|e| e.to_string())?;
    if document != *session.doc {
        return Err("container round-trip changed feature history".into());
    }
    let mut reopened = Session::new(document);
    if reopened.model.results.iter().any(|r| r.error.is_some()) {
        return Err("saved history failed to regenerate".into());
    }
    let remeasured = reopened.execute("inspect.measure", &json!({})).map_err(|e| e.to_string())?;
    let after = remeasured["total"]["volume_mm3"].as_f64().ok_or("missing reopened volume")?;
    if !after.is_finite() || (after - volume).abs() > volume * 1e-6 {
        return Err(format!("reopening changed volume: {volume} -> {after}"));
    }
    if path.file_name().is_some_and(|n| n == "Minimal.f3d") && (volume - std::f64::consts::PI * 50. * 50. * 100.).abs() > volume * 1e-3 {
        return Err("Minimal disagrees with its analytic cylinder volume".into());
    }
    Ok((session.doc.features.len(), volume))
}

#[test]
#[ignore = "full-import acceptance gate; requires user F3D files and currently exposes unsupported histories"]
fn supplied_archives_open_and_reopen_with_editable_history() {
    let root = solidcraft_engine::geom::env::var_os("F3D_CORPUS").expect("set SOLIDCRAFT_F3D_CORPUS to the user test folder");
    let mut paths = Vec::new();
    files(Path::new(&root), 0, &mut paths);
    // Optional folder for the F3D members extracted from user-supplied ZIP packs.
    if let Some(extra) = solidcraft_engine::geom::env::var_os("F3D_ADDITIONAL_CORPUS") {
        files(Path::new(&extra), 0, &mut paths);
    }
    paths.sort();
    paths.dedup();
    assert!(!paths.is_empty(), "no F3D archives found; a missing corpus must not pass");
    let mut failures = Vec::new();
    for path in &paths {
        match check(path) {
            Ok((features, volume)) => println!("PASS {}: {features} features, {volume:.6} mm3", path.display()),
            Err(error) => failures.push(format!("{}: {error}", path.display())),
        }
    }
    println!("{} / {} archives opened and reopened", paths.len() - failures.len(), paths.len());
    assert!(failures.is_empty(), "incomplete native imports:\n{}", failures.join("\n"));
}

#[test]
#[ignore = "requires the local user F3D corpus"]
fn supplied_archives_recover_with_both_preferences() {
    let root = solidcraft_engine::geom::env::var_os("F3D_CORPUS").expect("set SOLIDCRAFT_F3D_CORPUS");
    let mut paths = Vec::new();
    files(Path::new(&root), 0, &mut paths);
    if let Some(extra) = solidcraft_engine::geom::env::var_os("F3D_ADDITIONAL_CORPUS") {
        files(Path::new(&extra), 0, &mut paths);
    }
    paths.sort();
    paths.dedup();
    assert!(!paths.is_empty());
    for preference in ["editability", "correctness"] {
        for path in &paths {
            let mut session = Session::default();
            let began = std::time::Instant::now();
            println!("BEGIN {preference}: {}", path.display());
            let result = session
                .execute("doc.open", &json!({"path":path,"f3d_preference":preference}))
                .unwrap_or_else(|e| panic!("{} / {preference}: {e}", path.display()));
            println!("OPEN {:.2}s: {} features", began.elapsed().as_secs_f64(), session.doc.features.len());
            assert!(session.model.results.iter().all(|r| r.error.is_none()));
            if session.doc.features.is_empty() {
                assert!(!result["warnings"].as_array().unwrap().is_empty());
            }
            assert!(!session.model.state().bodies.is_empty(), "no recovered geometry: {} / {preference}", path.display());
            if path.file_name().is_some_and(|n| n == "Gridfinity Storage Box Shelves (Parametric) V1.0.f3d") {
                assert_eq!(session.model.state().bodies.len(), 6, "retain every recovered Gridfinity body");
                assert_eq!(result["representation"], "native_bodies");
                assert!(session.doc.import_warnings.iter().any(|warning| warning.contains("without historical topology")));
                assert!(
                    session.doc.import_warnings.iter().any(|warning| warning.contains("open body")),
                    "known incomplete shells must remain explicit"
                );
            }
            let volume: f64 = session.model.state().bodies.iter().map(|b| b.mesh().measure().volume).sum();
            if path.file_name().is_some_and(|n| n == "Minimal.f3d") {
                let measured: f64 = session.model.state().bodies.iter().map(|b| solidcraft_engine::kernel::measure(&b.body).unwrap().volume).sum();
                let expected = std::f64::consts::PI * 50. * 50. * 100.;
                assert!((measured - expected).abs() < expected * 1e-3, "Minimal analytic volume ({preference}): {measured}");
            }
            let bytes = io::write_design_container(&session.doc).unwrap();
            let reopened = Session::new(io::read_design(&bytes).unwrap());
            assert_eq!(session.doc, reopened.doc);
            assert!(reopened.model.results.iter().all(|r| r.error.is_none()));
            let after: f64 = reopened.model.state().bodies.iter().map(|b| b.mesh().measure().volume).sum();
            assert!(volume.is_finite() && after.is_finite() && (after - volume).abs() < volume.abs().max(1.) * 1e-6);
            println!(
                "RECOVER {preference}: {}: features={} bodies={} volume={volume:.3} representation={} warnings={}",
                path.display(),
                session.doc.features.len(),
                session.model.state().bodies.len(),
                result["representation"],
                session.doc.import_warnings.len()
            );
            for warning in session.doc.import_warnings.iter().take(5) {
                println!("  {warning}");
            }
        }
    }
}
