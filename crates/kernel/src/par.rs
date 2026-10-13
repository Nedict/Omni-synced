//! Ordered parallel map over independent items, on the process-wide worker pool (desktop
//! only; the browser build runs serially). Each item runs under [`crate::guard`], so a panic in
//! one item is that item's error and never reaches the pool.

use crate::{Result, guard};

/// `f` over `items`, results in item order. Runs serially below `min_items` items.
pub(crate) fn map<T: Sync, R: Send>(what: &'static str, items: &[T], min_items: usize, f: impl Fn(&T) -> Result<R> + Sync) -> Vec<Result<R>> {
    let one = |item: &T| guard(what, || f(item));
    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        if items.len() >= min_items.max(2) && rayon::current_num_threads() > 1 {
            // The pool's threads live for the whole process: handing out work costs
            // microseconds, where starting threads per call cost tens of them.
            return items.par_iter().map(one).collect();
        }
    }
    items.iter().map(one).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KernelError;

    #[test]
    fn keeps_order_and_errors() {
        let items: Vec<u32> = (0..1000).collect();
        let out = map("test", &items, 1, |x| if *x == 500 { Err(KernelError::Invalid("five hundred".into())) } else { Ok(x * 2) });
        assert_eq!(out.len(), 1000);
        for (i, r) in out.iter().enumerate() {
            match r {
                Ok(v) => assert_eq!(*v, 2 * i as u32),
                Err(_) => assert_eq!(i, 500),
            }
        }
    }
}
