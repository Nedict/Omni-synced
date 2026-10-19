//! How long the window thread spends on each frame, by what the app is doing (#3053): the window
//! must stay responsive through every phase of opening a design. `SOLIDCRAFT_PROFILE_FRAMES=1`
//! logs every frame slower than [`SLOW_MS`] with its phase; tests read [`Watch::slowest`].

use crate::SolveApp;

/// A frame slower than this is logged (ms).
pub(crate) const SLOW_MS: f64 = 50.0;

#[derive(Default)]
pub(crate) struct Watch {
    began: Option<f64>,
    phase: &'static str,
    /// The slowest frame so far: ms, and the phase it was in.
    pub slowest: (f64, &'static str),
}

/// What the app is doing this frame, as far as opening a design goes.
pub(crate) fn phase(app: &SolveApp) -> &'static str {
    if let Some(p) = crate::file_load::phase(app) {
        return p;
    }
    if crate::open_meshing::active(app) { "loading parts" } else { "idle" }
}

/// The frame starts (before any of its work).
pub(crate) fn begin(app: &mut SolveApp) {
    app.frame_watch.began = Some(crate::now_ms());
    app.frame_watch.phase = phase(app);
}

/// The frame's work is done.
pub(crate) fn end(app: &mut SolveApp) {
    let Some(began) = app.frame_watch.began.take() else { return };
    let ms = crate::now_ms() - began;
    // The phase the frame started in names it (a frame that finishes an open did that work).
    let phase = app.frame_watch.phase;
    if ms > app.frame_watch.slowest.0 {
        app.frame_watch.slowest = (ms, phase);
    }
    if ms > SLOW_MS && solidcraft_engine::geom::env::var_os("PROFILE_FRAMES").is_some() {
        eprintln!("SolidCraft slow frame: {ms:.0} ms ({phase})");
    }
}
