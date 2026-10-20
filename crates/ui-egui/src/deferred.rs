//! Commands that recompute the history, computed off the window thread as OK is (#1201): the
//! history marker dragged or clicked, a feature suppressed, reordered or deleted from the
//! timeline, undo and redo, the marker put back after a sketch. On a part with a slow feature
//! after the marker (a line of text cut into a block), each of these ran that feature's boolean
//! on the window thread, and the window froze until it was done.
//!
//! The command is evaluated on a copy of the session ([`solidcraft_engine::Session::ahead`]),
//! which leaves what it computes in the evaluation cache the session shares. Done within the
//! app's wait ([`crate::dialogs::APPLY_AT_ONCE`]), the command just runs; otherwise the window
//! goes on, saying it computes, and the command runs for real once the evaluation is in, taking
//! the features it evaluated ([`solidcraft_engine::Session::prime`]) rather than computing them
//! again. Commands asked for meanwhile wait their turn; a marker moved again
//! replaces the move still waiting, so a drag computes where it ends rather than every slot it
//! crossed.

use serde_json::Value;

use crate::SolveApp;

/// A command computing off the window thread.
pub struct Deferred {
    done: std::sync::mpsc::Receiver<Option<solidcraft_engine::Evaluated>>,
    /// What it evaluated, once in (`Some(None)`: it failed, or its thread is gone).
    evaluated: Option<Option<solidcraft_engine::Evaluated>>,
    /// When it started (ms).
    pub(crate) started: f64,
    command: (String, Value),
    /// The design's revision it was computed for: a design changed meanwhile computes it again.
    revision: u64,
}

impl Deferred {
    pub(crate) fn finished(&mut self) -> bool {
        if self.evaluated.is_none() {
            match self.done.try_recv() {
                Ok(evaluated) => self.evaluated = Some(evaluated),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.evaluated = Some(None),
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        self.evaluated.is_some()
    }
}

/// How a command asked to run off the window thread goes.
enum Start {
    /// It computes off the window thread.
    Later(Deferred),
    /// It runs now, with what was evaluated for it ahead (if anything).
    Now(Option<solidcraft_engine::Evaluated>),
}

/// Commands a later one of the same kind replaces while they wait: where the marker ends is all
/// that counts.
fn replaced_by_a_later_one(id: &str) -> bool {
    id == "timeline.roll_to"
}

impl SolveApp {
    /// Run a command that may recompute much of the history, off the window thread when it takes
    /// a while (see the module notes). `Ok(Null)` when it is left computing or waiting its turn:
    /// its outcome then shows as any command's does (an error in the status line).
    pub fn run_heavy(&mut self, id: &str, params: Value) -> Result<Value, String> {
        if self.deferred.is_some() {
            if replaced_by_a_later_one(id) && self.deferred_queue.last().is_some_and(|(q, _)| q == id) {
                self.deferred_queue.pop();
            }
            self.deferred_queue.push((id.to_string(), params));
            return Ok(Value::Null);
        }
        match self.begin_deferred(id, &params) {
            Start::Later(job) => {
                self.deferred = Some(job);
                Ok(Value::Null)
            }
            Start::Now(evaluated) => self.run_evaluated(id, params, evaluated.as_ref()),
        }
    }

    /// Run a command for real, taking the features `evaluated` computed for it ahead.
    fn run_evaluated(&mut self, id: &str, params: Value, evaluated: Option<&solidcraft_engine::Evaluated>) -> Result<Value, String> {
        if let Some(evaluated) = evaluated {
            self.session.prime(evaluated);
        }
        let r = self.run(id, params);
        self.session.unprime();
        r
    }

    /// Since when a command has been computing off the window thread (ms), if one is.
    pub fn computing_since(&self) -> Option<f64> {
        self.deferred.as_ref().map(|d| d.started)
    }

    /// Each frame: a command whose evaluation is in runs, then the next one waiting starts.
    pub(crate) fn poll_deferred(&mut self, ctx: &egui::Context) {
        if let Some(job) = &mut self.deferred {
            if !job.finished() {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
                return;
            }
            if let Some(job) = self.deferred.take() {
                let (id, params) = job.command;
                let superseded = replaced_by_a_later_one(&id) && self.deferred_queue.first().is_some_and(|(q, _)| *q == id);
                if job.revision != self.session.revision {
                    // The design changed meanwhile: computed again for the design as it is now.
                    let _ = self.run_heavy(&id, params);
                } else if !superseded {
                    let _ = self.run_evaluated(&id, params, job.evaluated.flatten().as_ref());
                }
            }
        }
        if self.deferred.is_none() && !self.deferred_queue.is_empty() {
            let (id, params) = self.deferred_queue.remove(0);
            let _ = self.run_heavy(&id, params);
        }
    }

    /// Start evaluating a command ahead. `None` when it should just run: not asked for
    /// ([`SolveApp::apply_off_thread`]), a file operation in progress, no threads (the web), or
    /// done within the wait.
    #[cfg(not(target_arch = "wasm32"))]
    fn begin_deferred(&self, id: &str, params: &Value) -> Start {
        let Some(wait) = self.apply_off_thread else { return Start::Now(None) };
        if self.file_load.busy() || self.file_picker.is_some() {
            return Start::Now(None);
        }
        let ahead = self.session.ahead();
        let command = (id.to_string(), params.clone());
        let commands = vec![command.clone()];
        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new().name("history".into()).spawn(move || {
            let _ = tx.send(ahead.evaluate(&commands).ok());
        });
        if spawned.is_err() {
            return Start::Now(None);
        }
        // Zero explicitly requests off-thread application, even if a fast worker
        // finishes before this thread is scheduled again. Apply on the next frame.
        if !wait.is_zero() {
            match rx.recv_timeout(wait) {
                Ok(evaluated) => return Start::Now(evaluated),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Start::Now(None),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
        Start::Later(Deferred { done: rx, evaluated: None, started: crate::now_ms(), command, revision: self.session.revision })
    }

    #[cfg(target_arch = "wasm32")]
    fn begin_deferred(&self, _id: &str, _params: &Value) -> Start {
        Start::Now(None)
    }
}
