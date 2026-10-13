//! Ordered parallel map over independent items, on the process-wide worker pool (desktop
//! only; the browser build runs serially). Each item runs under [`crate::guard`], so a panic in
//! one item is that item's error and never reaches the pool.

use crate::{Result, guard};

/// `f` over `items`, results in item order: on the pool when `parallel`, else on the calling
/// thread. Waking the pool's workers costs tens of microseconds, so callers ask for it only
/// when the items carry more work than that (decided from the work, not the item count).
pub(crate) fn map<T: Sync, R: Send>(what: &'static str, items: &[T], parallel: bool, f: impl Fn(&T) -> Result<R> + Sync) -> Vec<Result<R>> {
    let one = |item: &T| guard(what, || f(item));
    // The browser build has no worker pool: everything runs on the calling thread.
    #[cfg(target_arch = "wasm32")]
    let _ = parallel;
    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        if parallel && items.len() > 1 && rayon::current_num_threads() > 1 {
            return items.par_iter().map(one).collect();
        }
    }
    items.iter().map(one).collect()
}

/// Run `f` on every item, spread over plain threads (one per core, at most `max_threads`),
/// each pulling the next item; on the calling thread in the browser build or when threads
/// cannot be started. Each call runs under [`crate::guard`] (a panic is that item's loss).
///
/// Plain threads rather than the worker pool: an item may wait on a lazily filled cell that
/// another item is filling, and a pool worker that picked up a second item while it waited
/// inside its own item's parallel work could wait on itself.
pub fn for_each_on_threads<T: Sync>(items: &[T], max_threads: usize, f: impl Fn(&T) + Sync) {
    let one = |item: &T| {
        let _ = guard("parallel item", || {
            f(item);
            Ok(())
        });
    };
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let threads = std::thread::available_parallelism().map(usize::from).unwrap_or(1).min(max_threads).min(items.len());
        if threads > 1 {
            let next = AtomicUsize::new(0);
            let work = || {
                while let Some(item) = items.get(next.fetch_add(1, Ordering::Relaxed)) {
                    one(item);
                }
            };
            std::thread::scope(|scope| {
                for _ in 1..threads {
                    // A thread that cannot start leaves its share to the others.
                    let _ = std::thread::Builder::new().name("solidcraft-each".into()).spawn_scoped(scope, work);
                }
                work();
            });
            return;
        }
    }
    let _ = max_threads;
    items.iter().for_each(one);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KernelError;

    #[test]
    fn each_item_once_on_threads() {
        let items: Vec<u32> = (0..1000).collect();
        let seen: Vec<std::sync::atomic::AtomicU32> = (0..1000).map(|_| std::sync::atomic::AtomicU32::new(0)).collect();
        for_each_on_threads(&items, 8, |x| {
            seen[*x as usize].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            assert!(*x != 500, "a panicking item is that item's loss");
        });
        assert!(seen.iter().all(|s| s.load(std::sync::atomic::Ordering::Relaxed) == 1));
    }

    #[test]
    fn keeps_order_and_errors() {
        let items: Vec<u32> = (0..1000).collect();
        let out = map("test", &items, true, |x| if *x == 500 { Err(KernelError::Invalid("five hundred".into())) } else { Ok(x * 2) });
        assert_eq!(out.len(), 1000);
        for (i, r) in out.iter().enumerate() {
            match r {
                Ok(v) => assert_eq!(*v, 2 * i as u32),
                Err(_) => assert_eq!(i, 500),
            }
        }
    }
}
