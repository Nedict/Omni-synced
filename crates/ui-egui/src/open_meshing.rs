//! A design that has just opened shows its parts as they are meshed (#3053).
//!
//! Opening hands the design to the window once its timeline is evaluated (or taken from its
//! file) and placed; the display meshes are made afterwards, off the UI thread. Until they all
//! are, the viewport draws the bodies already meshed ([`shown_world`]), so the first parts show
//! within moments and the window stays responsive while the rest come in. The first placement of
//! every part is meshed first: its other placements only move that mesh. The view is framed from
//! the bodies' outlines meanwhile, and framed again from the meshes when they are all made, unless
//! the camera was moved in between. Editing the design meanwhile meshes the new world instead.
//!
//! Display only: commands, picking of what is drawn and exports read the session's world as ever.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use solidcraft_engine::doc::ModelState;

use crate::SolveApp;

/// How often a growing picture is rebuilt at most (s): each rebuild uploads every shown body.
const REFRESH: f64 = 0.25;

pub(crate) struct Meshing {
    /// The world being meshed (the session's when it started).
    world: Arc<ModelState>,
    /// Per body of `world`: its display (mesh, and plans when drawn from them) is made.
    ready: Arc<Vec<AtomicBool>>,
    done: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    /// The picture so far: the bodies shown, and how many were ready when it was made.
    shown: Option<(usize, Arc<ModelState>)>,
    shown_at: f64,
    /// The camera the outline framing left (a refit at the end only replaces that one).
    framed: Option<crate::Camera>,
}

impl Meshing {
    fn start(world: Arc<ModelState>, plans: bool) -> Meshing {
        let n = world.bodies.len();
        let ready: Arc<Vec<AtomicBool>> = Arc::new((0..n).map(|_| AtomicBool::new(false)).collect());
        let (done, stop) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicBool::new(false)));
        // The first placement of each part first: the others only move its mesh.
        let mut seen = std::collections::HashSet::new();
        let (mut first, mut rest) = (Vec::new(), Vec::new());
        for (i, b) in world.bodies.iter().enumerate() {
            if seen.insert(b.definition_name().to_string()) { first.push(i) } else { rest.push(i) }
        }
        first.extend(rest);
        let job = (world.clone(), ready.clone(), done.clone(), stop.clone());
        let spawned = std::thread::Builder::new().name("open-meshing".into()).spawn(move || {
            let (world, ready, done, stop) = job;
            solidcraft_engine::kernel::for_each_on_threads(&first, 16, |&i| {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                if let Some(b) = world.bodies.get(i) {
                    let _ = b.mesh();
                    if plans {
                        let _ = b.display_plan();
                        let _ = b.plan_mesh();
                    }
                }
                if let Some(r) = ready.get(i) {
                    r.store(true, Ordering::Release);
                }
                done.fetch_add(1, Ordering::Release);
            });
        });
        if spawned.is_err() {
            // No thread: the viewport meshes as it draws, as before.
            done.store(n, Ordering::Release);
        }
        Meshing { world, ready, done, stop, shown: None, shown_at: f64::NEG_INFINITY, framed: None }
    }

    fn finished(&self) -> bool {
        self.done.load(Ordering::Acquire) >= self.world.bodies.len()
    }
}

impl Drop for Meshing {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Start meshing the session's world off the UI thread (an opened design), framing the view from
/// the bodies' outlines.
pub(crate) fn start(app: &mut SolveApp) {
    let plans = app.viewport.gpu.is_some_and(|g| g.tess);
    let meshing = Meshing::start(app.session.world_state(), plans);
    let done = meshing.finished();
    app.open_meshing = Some(meshing);
    app.fit_view();
    if let Some(m) = app.open_meshing.as_mut() {
        m.framed = Some(app.cam);
    }
    if done {
        app.open_meshing = None;
    }
}

/// Whether bodies are still being meshed after an open.
pub(crate) fn active(app: &SolveApp) -> bool {
    app.open_meshing.is_some()
}

/// Per frame: follow the session's world, show more as it is meshed, and frame the finished
/// picture.
pub(crate) fn tick(app: &mut SolveApp, ctx: &egui::Context) {
    let Some(m) = app.open_meshing.as_ref() else { return };
    let world = app.session.world_state();
    if !Arc::ptr_eq(&world, &m.world) {
        // The design changed (an edit, an undo): mesh the new world; what is made is kept.
        let framed = m.framed;
        let plans = app.viewport.gpu.is_some_and(|g| g.tess);
        let mut next = Meshing::start(world, plans);
        next.framed = framed;
        app.open_meshing = Some(next);
    }
    let finished = app.open_meshing.as_ref().is_some_and(Meshing::finished);
    if finished {
        let framed = app.open_meshing.take().and_then(|m| m.framed);
        // Framed from outlines and not moved since: frame the meshes.
        if framed == Some(app.cam) {
            app.fit_view();
        }
    }
    ctx.request_repaint();
}

/// The world the viewport draws: while an opened design is still being meshed, the bodies meshed
/// so far (the picture grows a few times a second); otherwise the session's.
pub(crate) fn shown_world(app: &SolveApp) -> Arc<ModelState> {
    let Some(m) = app.open_meshing.as_ref() else { return app.session.world_state() };
    if !Arc::ptr_eq(&app.session.world_state(), &m.world) {
        return app.session.world_state();
    }
    m.shown.as_ref().map(|(_, st)| st.clone()).unwrap_or_else(|| Arc::new(ModelState::default()))
}

/// Remake the picture so far, at most every [`REFRESH`] seconds (each remake uploads it again).
pub(crate) fn refresh(app: &mut SolveApp, now: f64) {
    let Some(m) = app.open_meshing.as_mut() else { return };
    let done = m.done.load(Ordering::Acquire);
    if m.shown.as_ref().is_some_and(|(n, _)| *n == done) || now - m.shown_at < REFRESH {
        return;
    }
    let mut st = (*m.world).clone();
    let mut i = 0;
    st.bodies.retain(|_| {
        let keep = m.ready.get(i).is_some_and(|r| r.load(Ordering::Acquire));
        i += 1;
        keep
    });
    m.shown = Some((done, Arc::new(st)));
    m.shown_at = now;
}
