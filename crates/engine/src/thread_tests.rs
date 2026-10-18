//! Modelled threads: the ISO metric profile cut along a helix, M3 to M20, outside and inside.

use std::f64::consts::PI;

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    match s.execute(id, &p) {
        Ok(v) => v,
        Err(e) => panic!("{id} {p}: {e}"),
    }
}

/// Volume from the body's mesh: exact for the faceted threaded body, and quicker than a full
/// measurement of its thousands of faces.
fn volume(s: &mut Session, b: &str) -> f64 {
    let st = s.model.state();
    st.body(b).map(|mb| mb.body.tessellate(1e-3).unwrap().measure().volume).unwrap_or(0.0)
}

const SIZES: [(f64, f64); 9] = [(3.0, 0.5), (4.0, 0.7), (5.0, 0.8), (6.0, 1.0), (8.0, 1.25), (10.0, 1.5), (12.0, 1.75), (16.0, 2.0), (20.0, 2.5)];

/// ∫ 2π r · width(r) dr over [a, b]: the volume one turn of the groove takes out (a screw
/// motion of a section in an axial plane sweeps 2π∫r dA per turn).
fn per_turn(a: f64, b: f64, width: impl Fn(f64) -> f64) -> f64 {
    let n = 400;
    (0..n).map(|i| a + (b - a) * (i as f64 + 0.5) / n as f64).map(|r| 2.0 * PI * r * width(r) * (b - a) / n as f64).sum()
}

#[test]
fn modelled_external_threads_m3_to_m20() {
    let h = |p: f64| p * 3f64.sqrt() / 2.0;
    let slope = (PI / 6.0).tan();
    for (d, p) in SIZES {
        let mut s = Session::default();
        let len = 4.0 * p;
        run(&mut s, "solid.cylinder", json!({"radius": d / 2.0, "height": len + 2.0, "body_name": "Shaft"}));
        let before = volume(&mut s, "Shaft");
        let th = run(&mut s, "solid.thread", json!({"face": [d / 2.0, 0, 1], "length": len, "modeled": true}));
        assert!(th["thread"].is_null(), "a modelled thread is real geometry, not an annotation: {th}");
        let r_root = d / 2.0 - 5.0 * h(p) / 8.0;
        let want = per_turn(r_root, d / 2.0, |r| p / 4.0 + 2.0 * (r - r_root) * slope) * len / p;
        let got = before - volume(&mut s, "Shaft");
        assert!((got - want).abs() < want * 0.03, "M{d}: removed {got}, ISO profile {want}");
    }
}

#[test]
fn modelled_thread_section_shows_iso_flank_and_pitch() {
    let mut s = Session::default();
    let (d, p) = (8.0, 1.25);
    let len = 2.0 * p;
    run(&mut s, "solid.cylinder", json!({"radius": d / 2.0, "height": len + 2.0, "body_name": "Shaft"}));
    run(&mut s, "solid.thread", json!({"face": [d / 2.0, 0.0, 1.0], "length": len, "modeled": true}));
    let body = s.model.state().bodies.iter().find(|b| b.name == "Shaft").unwrap().body.clone();
    let mesh = body.tessellate(0.15).unwrap();
    // Unwrap the screw: radius against (height − pitch · angle) is the axial profile.
    // The ISO flank is 30° to the plane normal to the axis, and it repeats every pitch.
    let profile = |pitch: f64, sign: f64| -> Vec<f64> {
        let n = 16;
        let mut best = vec![f64::NAN; n];
        let mut cnt = vec![0u32; n];
        for q in &mesh.positions {
            if q.z < 0.5 || q.z > 2.2 {
                continue;
            }
            let r = (q.x * q.x + q.y * q.y).sqrt();
            if !(r > 2.8 && r < 4.15) {
                continue;
            }
            let s = (q.z - sign * pitch * q.y.atan2(q.x) / std::f64::consts::TAU).rem_euclid(pitch);
            let i = ((s / pitch) * n as f64) as usize % n;
            let slot = best.get_mut(i).and_then(|b| cnt.get_mut(i).map(|c| (b, c)));
            if let Some((b, c)) = slot {
                *b = if c == &0 { r } else { b.min(r) };
                *c += 1;
            }
        }
        best
    };
    let spread = |bins: &[f64]| {
        let v: Vec<f64> = bins.iter().copied().filter(|r| r.is_finite()).collect();
        v.iter().fold((f64::MAX, 0.0_f64), |(lo, hi), r| (lo.min(*r), hi.max(*r))).1 - v.iter().fold(f64::MAX, |lo, r| lo.min(*r))
    };
    let right = profile(p, 1.0);
    let left = profile(p, -1.0);
    let bins = if spread(&right) >= spread(&left) { right } else { left };
    let depth = spread(&bins);
    assert!(depth > 0.45, "profile depth {depth}, bins {bins:?}");
    let mut steep: f64 = 0.0;
    let ds = p / bins.len() as f64;
    for w in bins.windows(2) {
        if let [a, b] = w
            && a.is_finite()
            && b.is_finite()
        {
            steep = steep.max(((b - a) / ds).abs());
        }
    }
    // One flank rises √3 radially per unit along the axis (60° included angle).
    let flank = 3f64.sqrt();
    assert!((steep - flank).abs() < 0.25, "flank slope {steep} vs {flank}, bins {bins:?}");
    let muddy = spread(&profile(p * 1.7, 1.0)).max(spread(&profile(p * 1.7, -1.0)));
    assert!(depth > muddy * 1.4, "pitch {p} stands out ({depth}, muddy {muddy})");
}

#[test]
fn cosmetic_thread_does_not_cut_and_an_oversize_modelled_thread_is_refused() {
    let mut s = Session::default();
    run(&mut s, "solid.cylinder", json!({"radius": 4.0, "height": 12.0, "body_name": "Shaft"}));
    let before = volume(&mut s, "Shaft");
    let th = run(&mut s, "solid.thread", json!({"face": [4.0, 0.0, 2.0], "length": 6.0}));
    assert!(th["thread"].is_object(), "{th}");
    assert!((volume(&mut s, "Shaft") - before).abs() < 1e-6);
    let features = s.doc.features.len();
    let err = s.execute("solid.thread", &json!({"face": [4.0, 0.0, 2.0], "designation": "M24", "modeled": true})).unwrap_err();
    assert!(err.to_string().contains("not supported yet"), "{err}");
    assert_eq!(s.doc.features.len(), features);
    assert!((volume(&mut s, "Shaft") - before).abs() < 1e-6);
    let err = s.execute("solid.thread", &json!({"face": [0.0, 0.0, 12.0], "modeled": true})).unwrap_err();
    assert!(err.to_string().contains("not supported yet"), "{err}");
    assert_eq!(s.doc.features.len(), features);
}

#[test]
fn modelled_internal_threads_m3_to_m20() {
    let h = |p: f64| p * 3f64.sqrt() / 2.0;
    let slope = (PI / 6.0).tan();
    for (d, p) in SIZES {
        let mut s = Session::default();
        let len = 4.0 * p;
        let d1 = d - 2.0 * 5.0 * h(p) / 8.0;
        run(&mut s, "solid.box", json!({"corner": [-d, -d, 0], "length": 2.0 * d, "width": 2.0 * d, "height": len, "body_name": "Nut"}));
        run(&mut s, "solid.cylinder", json!({"base": [0, 0, -1], "radius": d1 / 2.0, "height": len + 2.0, "body_name": "Bore"}));
        run(&mut s, "solid.combine", json!({"target": "Nut", "tools": ["Bore"], "operation": "cut"}));
        let before = volume(&mut s, "Nut");
        let th = run(&mut s, "solid.thread", json!({"face": [d1 / 2.0, 0, len / 2.0], "modeled": true}));
        assert!(th["thread"].is_null(), "{th}");
        // Bolt-shaped space: P/8 flat at the major diameter, widening towards the bore.
        let want = per_turn(d1 / 2.0, d / 2.0, |r| p / 8.0 + 2.0 * (d / 2.0 - r) * slope) * len / p;
        let got = before - volume(&mut s, "Nut");
        assert!((got - want).abs() < want * 0.03, "M{d}: removed {got}, ISO profile {want}");
    }
}
