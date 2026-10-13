//! Progress and cooperative cancellation for a long evaluation (opening a large design) run
//! off the window thread. The caller wraps the work in [`watch`]; the timeline's evaluation
//! loop reports each feature it reaches and, once [`Progress::cancel`] is called, skips the
//! rest. Nothing changes for evaluations that are not watched.

use std::cell::RefCell;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

/// Shared between the worker evaluating a design and the window showing its progress.
#[derive(Debug, Default)]
pub struct Progress {
    cancelled: AtomicBool,
    done: AtomicUsize,
    total: AtomicUsize,
}

impl Progress {
    /// Ask the watched evaluation to stop at its next feature.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
    /// Features evaluated so far and the timeline's length (0 before evaluation starts).
    pub fn features(&self) -> (usize, usize) {
        (self.done.load(Ordering::Relaxed), self.total.load(Ordering::Relaxed))
    }
}

struct Current {
    progress: Arc<Progress>,
    /// Nested evaluations (derived designs) don't overwrite the outer timeline's count.
    depth: usize,
}

thread_local! {
    static CURRENT: RefCell<Option<Current>> = const { RefCell::new(None) };
}

/// Run `f` with `progress` watching every evaluation it makes on this thread.
pub fn watch<R>(progress: Arc<Progress>, f: impl FnOnce() -> R) -> R {
    let previous = CURRENT.with(|c| c.replace(Some(Current { progress, depth: 0 })));
    struct Restore(Option<Option<Current>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            if let Some(previous) = self.0.take() {
                CURRENT.with(|c| *c.borrow_mut() = previous);
            }
        }
    }
    let _restore = Restore(Some(previous));
    f()
}

/// Entered at the start of an evaluation; the guard leaves it.
pub(crate) struct Evaluation {
    outer: bool,
}

pub(crate) fn begin(total: usize) -> Evaluation {
    let outer = CURRENT.with(|c| {
        let mut c = c.borrow_mut();
        let Some(cur) = c.as_mut() else { return false };
        cur.depth += 1;
        if cur.depth == 1 {
            cur.progress.total.store(total, Ordering::Relaxed);
            cur.progress.done.store(0, Ordering::Relaxed);
            true
        } else {
            false
        }
    });
    Evaluation { outer }
}

impl Evaluation {
    /// Feature `index` is next. True when the watcher asked to stop: skip it.
    pub(crate) fn step(&self, index: usize) -> bool {
        CURRENT.with(|c| {
            let c = c.borrow();
            let Some(cur) = c.as_ref() else { return false };
            if self.outer {
                cur.progress.done.store(index, Ordering::Relaxed);
            }
            cur.progress.is_cancelled()
        })
    }
}

impl Drop for Evaluation {
    fn drop(&mut self) {
        CURRENT.with(|c| {
            if let Some(cur) = c.borrow_mut().as_mut() {
                cur.depth = cur.depth.saturating_sub(1);
                if self.outer {
                    let total = cur.progress.total.load(Ordering::Relaxed);
                    cur.progress.done.store(total, Ordering::Relaxed);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwatched_evaluations_never_stop_and_watched_ones_report_and_stop() {
        let e = begin(3);
        assert!(!e.step(0));
        drop(e);
        let p = Arc::new(Progress::default());
        watch(p.clone(), || {
            let e = begin(5);
            assert!(!e.step(2));
            assert_eq!(p.features(), (2, 5));
            {
                let inner = begin(100);
                assert!(!inner.step(50));
            }
            assert_eq!(p.features(), (2, 5), "a nested evaluation keeps the outer count");
            p.cancel();
            assert!(e.step(3));
        });
        assert_eq!(p.features(), (5, 5));
        assert!(!begin(1).step(0), "the watch ends with its scope");
    }
}
